//! Explicit hardware commands, separate from document history.
use crate::{Effect, EngineError};
use serde::{Deserialize, Serialize};
use stencilcraft_machine as machine;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", content = "params")]
pub enum HardwareCommand {
    /// Enumerate USB descriptors without opening or moving a cutter.
    #[serde(rename = "hardware.devices")]
    Devices,
    /// Validate the current visible document; only send when `send` is true.
    #[serde(rename = "hardware.cut")]
    Cut {
        model: String,
        location: String,
        mat: machine::Mat,
        force: u32,
        speed: u32,
        #[serde(default)]
        send: bool,
    },
}

pub fn run(
    doc: &stencilcraft_doc::Document,
    command: &HardwareCommand,
) -> Result<Effect, EngineError> {
    execute(doc, command)
        .map(Effect::Hardware)
        .map_err(|e| EngineError::Invalid(e.to_string()))
}

fn execute(
    doc: &stencilcraft_doc::Document,
    command: &HardwareCommand,
) -> Result<serde_json::Value, machine::MachineError> {
    let HardwareCommand::Cut {
        model,
        location,
        mat,
        force,
        speed,
        send,
    } = command
    else {
        return devices();
    };
    let caps = machine::DeviceRegistry::builtin()
        .by_id(model)
        .ok_or_else(|| machine::MachineError::Config(format!("Unknown cutter {model}")))?;
    let mut paths = Vec::new();
    for layer in doc.layers.iter().filter(|l| l.visible) {
        for shape in layer.shapes.iter().filter(|s| !s.hidden) {
            for line in stencilcraft_geom::path_to_polylines(
                &shape.path,
                stencilcraft_geom::DEFAULT_FLATTEN_STEPS,
            ) {
                let mut points: Vec<_> = line.points.iter().map(|p| (p.x, p.y)).collect();
                if points.len() < 2 {
                    continue;
                }
                if line.closed
                    && let Some(first) = points.first().copied()
                {
                    points.push(first);
                }
                paths.push(points);
            }
        }
    }
    if paths.is_empty() {
        return Err(machine::MachineError::Config(
            "No visible paths to cut".into(),
        ));
    }
    let job = machine::CutJob {
        mat: *mat,
        operations: vec![machine::Operation {
            holder: 1,
            paths,
            settings: machine::OpSettings {
                force: Some(*force),
                speed: Some(*speed),
                ..Default::default()
            },
            ..Default::default()
        }],
        ..Default::default()
    };
    let program = machine::dry_run(caps, &job)?;
    if !send {
        return Ok(
            serde_json::json!({"model":model,"bytes":program.to_stream().len(),"sent":false}),
        );
    }
    send_usb(caps, location, &job)
}

#[cfg(all(feature = "usb", not(target_arch = "wasm32")))]
fn devices() -> Result<serde_json::Value, machine::MachineError> {
    Ok(serde_json::Value::Array(machine::list_usb_devices()?.into_iter().map(|d| {
        let caps = machine::DeviceRegistry::builtin().by_usb(d.vendor_id, d.product_id);
        serde_json::json!({"location":d.location,"product":d.product,"model":caps.map(|c| &c.id),"name":caps.map(|c| &c.name)})
    }).collect()))
}
#[cfg(not(all(feature = "usb", not(target_arch = "wasm32"))))]
fn devices() -> Result<serde_json::Value, machine::MachineError> {
    Err(machine::MachineError::Config(
        "USB support is disabled in this build".into(),
    ))
}
#[cfg(all(feature = "usb", not(target_arch = "wasm32")))]
fn send_usb(
    caps: &machine::Capabilities,
    location: &str,
    job: &machine::CutJob,
) -> Result<serde_json::Value, machine::MachineError> {
    let devices = machine::list_usb_devices()?;
    let device = devices
        .iter()
        .find(|d| d.location == location)
        .ok_or_else(|| {
            machine::MachineError::Config("Selected cutter disconnected; refresh devices".into())
        })?;
    if machine::DeviceRegistry::builtin().by_usb(device.vendor_id, device.product_id) != Some(caps)
    {
        return Err(machine::MachineError::Config(
            "Selected USB device does not match the job model".into(),
        ));
    }
    let transport = machine::UsbTransport::open(Some(location))?;
    let mut driver = machine::MachineDriver::new(caps.clone(), Box::new(transport));
    let report = driver.send(job, &mut machine::NoHooks)?;
    Ok(serde_json::json!({"sent":true,"bytes":report.bytes,"firmware":report.version}))
}
#[cfg(not(all(feature = "usb", not(target_arch = "wasm32"))))]
fn send_usb(
    _: &machine::Capabilities,
    _: &str,
    _: &machine::CutJob,
) -> Result<serde_json::Value, machine::MachineError> {
    devices()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_cut_refused_before_usb() {
        let command = HardwareCommand::Cut {
            model: "cameo-4-pro".into(),
            location: "missing".into(),
            mat: machine::Mat::None,
            force: 10,
            speed: 5,
            send: true,
        };
        assert!(run(&stencilcraft_doc::Document::default(), &command).is_err());
    }
    #[test]
    fn validates_limits_without_opening_usb() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = crate::Engine::new(stencilcraft_doc::Document::default());
        engine.run(crate::Command::ShapeAddRect {
            x: 10.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
        })?;
        let mut command = HardwareCommand::Cut {
            model: "cameo-4".into(),
            location: "not-connected".into(),
            mat: machine::Mat::Cameo12x12,
            force: 10,
            speed: 5,
            send: false,
        };
        assert!(run(&engine.doc, &command).is_ok());
        if let HardwareCommand::Cut { force, .. } = &mut command {
            *force = 1000;
        }
        assert!(run(&engine.doc, &command).is_err());
        Ok(())
    }
    #[test]
    fn send_defaults_to_false() -> Result<(), Box<dyn std::error::Error>> {
        let command: HardwareCommand = serde_json::from_str(
            r#"{"command":"hardware.cut","params":{"model":"cameo-4-pro","location":"1-2","mat":"none","force":10,"speed":5}}"#,
        )?;
        assert!(matches!(command, HardwareCommand::Cut { send: false, .. }));
        Ok(())
    }
}
