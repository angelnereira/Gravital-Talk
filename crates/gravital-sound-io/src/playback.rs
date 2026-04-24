//! Reproducción de audio en un output device.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use crate::devices::pick_output;
use crate::error::IoError;
use crate::{Result, StreamConfig};

pub struct AudioPlayback {
    _stream: Stream,
    tx: Sender<Vec<i16>>,
}

impl core::fmt::Debug for AudioPlayback {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AudioPlayback").finish_non_exhaustive()
    }
}

impl AudioPlayback {
    pub fn start(config: StreamConfig, device_hint: Option<&str>) -> Result<Self> {
        let device = pick_output(device_hint)?;
        let supported = device.default_output_config().map_err(IoError::from)?;
        let sample_format = supported.sample_format();
        let actual_config = supported.config();
        let _ = config;

        let buffer: Arc<Mutex<VecDeque<i16>>> = Arc::new(Mutex::new(VecDeque::with_capacity(8192)));
        let (tx, rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = mpsc::channel();
        let buf_for_pump = buffer.clone();

        // Pump thread: drena el Receiver y empuja al buffer compartido.
        std::thread::Builder::new()
            .name("gs-playback-pump".into())
            .spawn(move || {
                while let Ok(frame) = rx.recv() {
                    let mut guard = match buf_for_pump.lock() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    guard.extend(frame);
                }
            })
            .expect("spawn pump thread");

        let err_fn = |e| tracing::error!(%e, "cpal output stream error");
        let buf_for_cb = buffer.clone();

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &actual_config,
                move |data: &mut [f32], _: &_| {
                    pull_f32(data, &buf_for_cb);
                },
                err_fn,
                None,
            )?,
            SampleFormat::I16 => device.build_output_stream(
                &actual_config,
                move |data: &mut [i16], _: &_| {
                    pull_i16(data, &buf_for_cb);
                },
                err_fn,
                None,
            )?,
            SampleFormat::U16 => device.build_output_stream(
                &actual_config,
                move |data: &mut [u16], _: &_| {
                    pull_u16(data, &buf_for_cb);
                },
                err_fn,
                None,
            )?,
            other => {
                return Err(IoError::UnsupportedConfig(format!(
                    "sample format {other:?} not handled"
                )))
            }
        };

        stream.play()?;
        Ok(Self {
            _stream: stream,
            tx,
        })
    }

    pub fn sender(&self) -> Sender<Vec<i16>> {
        self.tx.clone()
    }

    pub fn push(&self, samples: Vec<i16>) -> Result<()> {
        self.tx.send(samples).map_err(|_| IoError::ChannelClosed)
    }
}

fn pull_f32(data: &mut [f32], buffer: &Mutex<VecDeque<i16>>) {
    let mut guard = match buffer.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    for slot in data.iter_mut() {
        *slot = guard
            .pop_front()
            .map(|s| s as f32 / i16::MAX as f32)
            .unwrap_or(0.0);
    }
}

fn pull_i16(data: &mut [i16], buffer: &Mutex<VecDeque<i16>>) {
    let mut guard = match buffer.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    for slot in data.iter_mut() {
        *slot = guard.pop_front().unwrap_or(0);
    }
}

fn pull_u16(data: &mut [u16], buffer: &Mutex<VecDeque<i16>>) {
    let mut guard = match buffer.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    for slot in data.iter_mut() {
        let s = guard.pop_front().unwrap_or(0);
        *slot = (s as i32 + 0x8000) as u16;
    }
}
