//! Captura de audio desde un input device.
//!
//! El stream callback convierte los samples f32 (formato nativo de cpal) a
//! i16 y los empuja por un `mpsc::Sender` hacia el consumer Gravital Sound.

use std::sync::mpsc::{self, Receiver, Sender};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use crate::devices::pick_input;
use crate::error::IoError;
use crate::{Result, StreamConfig};

pub struct AudioCapture {
    _stream: Stream,
}

impl core::fmt::Debug for AudioCapture {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AudioCapture").finish_non_exhaustive()
    }
}

impl AudioCapture {
    /// Abre el device y arranca la captura. Devuelve un `Receiver` que
    /// entrega vectores i16 cuyo tamaño = `config.samples_per_frame()`.
    ///
    /// Si cpal reporta un sample rate distinto al pedido, se intenta el
    /// más cercano soportado. Para sample-rate mismatch real recomendamos
    /// usar el default del device y reencauzar en el caller.
    pub fn start(
        config: StreamConfig,
        device_hint: Option<&str>,
    ) -> Result<(Self, Receiver<Vec<i16>>)> {
        let device = pick_input(device_hint)?;
        let supported = device.default_input_config().map_err(IoError::from)?;
        let sample_format = supported.sample_format();
        let actual_config = supported.config();

        if actual_config.channels as u8 != config.channels {
            tracing::warn!(
                requested = config.channels,
                actual = actual_config.channels,
                "input channels mismatch; using device default"
            );
        }
        if actual_config.sample_rate.0 != config.sample_rate {
            tracing::warn!(
                requested = config.sample_rate,
                actual = actual_config.sample_rate.0,
                "input sample rate mismatch; using device default"
            );
        }

        let frame_samples =
            (actual_config.sample_rate.0 as usize * config.frame_duration_ms as usize) / 1000
                * actual_config.channels as usize;

        let (tx, rx) = mpsc::channel();
        let mut accum: Vec<i16> = Vec::with_capacity(frame_samples);
        let tx_clone = tx.clone();

        let err_fn = |e| tracing::error!(%e, "cpal input stream error");

        let stream = match sample_format {
            SampleFormat::F32 => device.build_input_stream(
                &actual_config,
                move |data: &[f32], _: &_| {
                    push_f32(data, &mut accum, frame_samples, &tx_clone);
                },
                err_fn,
                None,
            )?,
            SampleFormat::I16 => device.build_input_stream(
                &actual_config,
                move |data: &[i16], _: &_| {
                    push_i16(data, &mut accum, frame_samples, &tx_clone);
                },
                err_fn,
                None,
            )?,
            SampleFormat::U16 => device.build_input_stream(
                &actual_config,
                move |data: &[u16], _: &_| {
                    push_u16(data, &mut accum, frame_samples, &tx_clone);
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
        Ok((Self { _stream: stream }, rx))
    }
}

fn push_f32(data: &[f32], accum: &mut Vec<i16>, frame_samples: usize, tx: &Sender<Vec<i16>>) {
    for &s in data {
        let clipped = s.clamp(-1.0, 1.0);
        accum.push((clipped * i16::MAX as f32) as i16);
        if accum.len() >= frame_samples {
            let frame = accum.split_off(0);
            if tx.send(frame).is_err() {
                return;
            }
        }
    }
}

fn push_i16(data: &[i16], accum: &mut Vec<i16>, frame_samples: usize, tx: &Sender<Vec<i16>>) {
    accum.extend_from_slice(data);
    while accum.len() >= frame_samples {
        let rest = accum.split_off(frame_samples);
        let frame = core::mem::replace(accum, rest);
        if tx.send(frame).is_err() {
            return;
        }
    }
}

fn push_u16(data: &[u16], accum: &mut Vec<i16>, frame_samples: usize, tx: &Sender<Vec<i16>>) {
    for &u in data {
        accum.push(u.wrapping_sub(0x8000) as i16);
        if accum.len() >= frame_samples {
            let frame = accum.split_off(0);
            if tx.send(frame).is_err() {
                return;
            }
        }
    }
}
