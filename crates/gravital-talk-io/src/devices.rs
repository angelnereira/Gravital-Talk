//! Enumeración de input/output devices.

use cpal::traits::{DeviceTrait, HostTrait};

use crate::error::IoError;
use crate::Result;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
}

fn host() -> Result<cpal::Host> {
    let host = cpal::default_host();
    let _ = host.devices().map_err(IoError::from)?;
    Ok(host)
}

pub fn list_input_devices() -> Result<Vec<DeviceInfo>> {
    let host = host()?;
    let default_name = host.default_input_device().and_then(|d| d.name().ok());
    let mut out = Vec::new();
    for d in host.input_devices()? {
        let name = d.name().unwrap_or_else(|_| "<unnamed>".into());
        let is_default = default_name.as_deref() == Some(name.as_str());
        out.push(DeviceInfo { name, is_default });
    }
    Ok(out)
}

pub fn list_output_devices() -> Result<Vec<DeviceInfo>> {
    let host = host()?;
    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    let mut out = Vec::new();
    for d in host.output_devices()? {
        let name = d.name().unwrap_or_else(|_| "<unnamed>".into());
        let is_default = default_name.as_deref() == Some(name.as_str());
        out.push(DeviceInfo { name, is_default });
    }
    Ok(out)
}

pub(crate) fn pick_input(name_hint: Option<&str>) -> Result<cpal::Device> {
    let host = host()?;
    if let Some(hint) = name_hint {
        if hint == "default" {
            return host
                .default_input_device()
                .ok_or(IoError::NoDevice { kind: "input" });
        }
        for d in host.input_devices()? {
            if d.name().map(|n| n == hint).unwrap_or(false) {
                return Ok(d);
            }
        }
        return Err(IoError::NoDevice { kind: "input" });
    }
    host.default_input_device()
        .ok_or(IoError::NoDevice { kind: "input" })
}

pub(crate) fn pick_output(name_hint: Option<&str>) -> Result<cpal::Device> {
    let host = host()?;
    if let Some(hint) = name_hint {
        if hint == "default" {
            return host
                .default_output_device()
                .ok_or(IoError::NoDevice { kind: "output" });
        }
        for d in host.output_devices()? {
            if d.name().map(|n| n == hint).unwrap_or(false) {
                return Ok(d);
            }
        }
        return Err(IoError::NoDevice { kind: "output" });
    }
    host.default_output_device()
        .ok_or(IoError::NoDevice { kind: "output" })
}
