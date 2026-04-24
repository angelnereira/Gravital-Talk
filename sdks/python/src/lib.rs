//! PyO3 bindings para Gravital Sound.
//!
//! Exporta el módulo `_gravital_sound` con clases `Session`, `Config`,
//! `Metrics` envueltas sobre la facade Rust. La API de alto nivel Pythonic
//! vive en `gravital_sound/*.py`.

#![forbid(unsafe_op_in_unsafe_fn)]

use std::net::SocketAddr;
use std::sync::Arc;

use gravital_sound::{
    Config as RustConfig, MetricsSnapshot, Session as RustSession, SessionRole, UdpConfig,
    UdpTransport,
};
use once_cell::sync::Lazy;
use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;

static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .expect("tokio runtime")
});

#[pyclass(name = "Config")]
#[derive(Clone)]
struct PyConfig {
    inner: RustConfig,
}

#[pymethods]
impl PyConfig {
    #[new]
    #[pyo3(signature = (sample_rate=48_000, channels=1, frame_duration_ms=20, jitter_buffer_ms=40))]
    fn new(sample_rate: u32, channels: u8, frame_duration_ms: u8, jitter_buffer_ms: u16) -> Self {
        let mut c = RustConfig::default();
        c.sample_rate = sample_rate;
        c.channels = channels;
        c.frame_duration_ms = frame_duration_ms;
        c.jitter_buffer_ms = jitter_buffer_ms;
        Self { inner: c }
    }

    #[getter]
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate
    }
    #[getter]
    fn channels(&self) -> u8 {
        self.inner.channels
    }
    #[getter]
    fn frame_duration_ms(&self) -> u8 {
        self.inner.frame_duration_ms
    }
    #[getter]
    fn jitter_buffer_ms(&self) -> u16 {
        self.inner.jitter_buffer_ms
    }

    fn __repr__(&self) -> String {
        format!(
            "Config(sample_rate={}, channels={}, frame_duration_ms={}, jitter_buffer_ms={})",
            self.inner.sample_rate,
            self.inner.channels,
            self.inner.frame_duration_ms,
            self.inner.jitter_buffer_ms,
        )
    }
}

#[pyclass(name = "Metrics")]
#[derive(Clone)]
struct PyMetrics {
    snap: MetricsSnapshot,
}

#[pymethods]
impl PyMetrics {
    #[getter]
    fn rtt_ms(&self) -> f32 { self.snap.rtt_ms }
    #[getter]
    fn jitter_ms(&self) -> f32 { self.snap.jitter_ms }
    #[getter]
    fn loss_percent(&self) -> f32 { self.snap.loss_percent }
    #[getter]
    fn reorder_percent(&self) -> f32 { self.snap.reorder_percent }
    #[getter]
    fn buffer_fill_percent(&self) -> f32 { self.snap.buffer_fill_percent }
    #[getter]
    fn estimated_mos(&self) -> f32 { self.snap.estimated_mos }
    #[getter]
    fn packets_sent(&self) -> u64 { self.snap.packets_sent }
    #[getter]
    fn packets_received(&self) -> u64 { self.snap.packets_received }
    #[getter]
    fn bytes_sent(&self) -> u64 { self.snap.bytes_sent }
    #[getter]
    fn bytes_received(&self) -> u64 { self.snap.bytes_received }

    fn __repr__(&self) -> String {
        format!(
            "Metrics(mos={:.2}, rtt_ms={:.2}, loss%={:.2}, jitter_ms={:.2})",
            self.snap.estimated_mos,
            self.snap.rtt_ms,
            self.snap.loss_percent,
            self.snap.jitter_ms,
        )
    }
}

#[pyclass(name = "Session")]
struct PySession {
    inner: Arc<RustSession>,
    local_addr: SocketAddr,
}

#[pymethods]
impl PySession {
    #[new]
    #[pyo3(signature = (config=None, bind_addr="0.0.0.0", bind_port=0))]
    fn new(config: Option<PyConfig>, bind_addr: &str, bind_port: u16) -> PyResult<Self> {
        use gravital_sound::Transport;
        let cfg = config.map(|c| c.inner).unwrap_or_default();
        let ip: std::net::IpAddr = bind_addr
            .parse()
            .map_err(|e: std::net::AddrParseError| PyValueError::new_err(e.to_string()))?;
        let bind = SocketAddr::new(ip, bind_port);
        let transport = RUNTIME
            .block_on(UdpTransport::bind(UdpConfig {
                bind_addr: bind,
                ..Default::default()
            }))
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
        let local_addr = transport
            .local_addr()
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(RustSession::new(Arc::new(transport), cfg)),
            local_addr,
        })
    }

    fn connect(&self, py: Python<'_>, host: &str, port: u16) -> PyResult<()> {
        let peer: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e: std::net::AddrParseError| PyValueError::new_err(e.to_string()))?;
        let s = self.inner.clone();
        py.allow_threads(|| {
            RUNTIME
                .block_on(async move { s.handshake(SessionRole::Client, peer).await })
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))
        })
    }

    fn accept(&self, py: Python<'_>, host: &str, port: u16) -> PyResult<()> {
        let peer: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e: std::net::AddrParseError| PyValueError::new_err(e.to_string()))?;
        let s = self.inner.clone();
        py.allow_threads(|| {
            RUNTIME
                .block_on(async move { s.handshake(SessionRole::Server, peer).await })
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))
        })
    }

    fn send_audio(&self, py: Python<'_>, data: &[u8]) -> PyResult<()> {
        let s = self.inner.clone();
        let owned = data.to_vec();
        py.allow_threads(|| {
            RUNTIME
                .block_on(async move { s.send_audio(&owned).await })
                .map_err(|e| PyIOError::new_err(e.to_string()))
        })
    }

    fn recv_audio(&self, py: Python<'_>) -> PyResult<Py<pyo3::types::PyBytes>> {
        let s = self.inner.clone();
        let frame = py.allow_threads(|| {
            RUNTIME
                .block_on(async move { s.recv_audio().await })
                .map_err(|e| PyIOError::new_err(e.to_string()))
        })?;
        Python::with_gil(|py| Ok(pyo3::types::PyBytes::new(py, &frame.payload).unbind()))
    }

    fn close(&self, py: Python<'_>) -> PyResult<()> {
        let s = self.inner.clone();
        py.allow_threads(|| {
            RUNTIME
                .block_on(async move { s.close().await })
                .map_err(|e| PyIOError::new_err(e.to_string()))
        })
    }

    #[getter]
    fn session_id(&self) -> u32 {
        self.inner.session_id()
    }

    #[getter]
    fn local_port(&self) -> u16 {
        self.local_addr.port()
    }

    #[getter]
    fn local_addr(&self) -> String {
        self.local_addr.to_string()
    }

    fn metrics(&self) -> PyMetrics {
        let fill = self.inner.jitter_buffer().fill_percent();
        let snap = self.inner.metrics().snapshot(fill);
        PyMetrics { snap }
    }
}

/// Inicialización del módulo nativo.
#[pymodule]
fn _gravital_sound(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("PROTOCOL_VERSION", u32::from(gravital_sound::PROTOCOL_VERSION))?;
    m.add_class::<PyConfig>()?;
    m.add_class::<PyMetrics>()?;
    m.add_class::<PySession>()?;
    Ok(())
}
