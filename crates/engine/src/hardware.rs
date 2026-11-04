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
    /// Discover paired Bluetooth LE cutters from the system Bluetooth service.
    #[serde(rename = "hardware.bluetooth_devices")]
    BluetoothDevices,
    /// Validate the current visible document; only send when `send` is true.
    #[serde(rename = "hardware.cut")]
    Cut {
        model: String,
        location: String,
        mat: machine::Mat,
        force: u32,
        speed: u32,
        /// Optional AutoBlade depth (0–10); omitted leaves blade adjustment to the user.
        #[serde(default)]
        depth: Option<u8>,
        /// Tool installed in the selected holder; omitted uses the loaded tool.
        #[serde(default)]
        tool: Option<machine::ToolKind>,
        /// Use a pen to draw rather than a blade to cut.
        #[serde(default)]
        draw: bool,
        /// One-based holder number and number of passes.
        #[serde(default = "one")]
        holder: u8,
        #[serde(default = "one")]
        passes: u8,
        /// Mirror about the material width.
        #[serde(default)]
        mirror: bool,
        /// Optional Linux Bluetooth Classic RFCOMM endpoint, paired outside the app.
        #[serde(default)]
        bluetooth: Option<BluetoothTarget>,
        #[serde(default)]
        send: bool,
    },
}

/// A Bluetooth Classic endpoint. The RFCOMM channel comes from the device's service record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothTarget {
    pub address: String,
    #[serde(default = "one")]
    pub channel: u8,
    /// Use Bluetooth LE instead of Classic RFCOMM.
    #[serde(default)]
    pub ble: bool,
}

fn one() -> u8 {
    1
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
    if matches!(command, HardwareCommand::BluetoothDevices) {
        return bluetooth_devices();
    }
    let HardwareCommand::Cut {
        model,
        location,
        mat,
        force,
        speed,
        depth,
        tool,
        draw,
        holder,
        passes,
        mirror,
        bluetooth,
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
    let tool = tool.or_else(|| depth.map(|_| machine::ToolKind::AutoBlade));
    if *draw && tool.is_some_and(|t| t != machine::ToolKind::Pen) {
        return Err(machine::MachineError::Config(
            "Draw requires a pen or an unspecified tool".into(),
        ));
    }
    if !*draw && tool == Some(machine::ToolKind::Pen) {
        return Err(machine::MachineError::Config(
            "Select Draw when using a pen".into(),
        ));
    }
    if depth.is_some() && tool != Some(machine::ToolKind::AutoBlade) {
        return Err(machine::MachineError::Config(
            "Blade depth can only be set for AutoBlade".into(),
        ));
    }
    let job = machine::CutJob {
        mat: *mat,
        mirror: *mirror,
        media_size_mm: (*mat == machine::Mat::None)
            .then_some((doc.board.width_mm, doc.board.height_mm)),
        operations: vec![machine::Operation {
            holder: *holder,
            kind: if *draw {
                machine::OpKind::Draw
            } else {
                machine::OpKind::Cut
            },
            tool,
            paths,
            settings: machine::OpSettings {
                force: Some(*force),
                speed: Some(*speed),
                depth: *depth,
                passes: *passes,
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
    if let Some(target) = bluetooth {
        send_bluetooth(caps, target, &job)
    } else {
        send_usb(caps, location, &job)
    }
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

#[cfg(all(feature = "bluetooth", target_os = "linux"))]
fn bluetooth_devices() -> Result<serde_json::Value, machine::MachineError> {
    let devices = machine::list_ble_devices()?;
    Ok(
        serde_json::json!({"bluetooth_devices": devices.into_iter().map(|(address, model, name)| serde_json::json!({"location":address,"model":model,"name":name})).collect::<Vec<_>>()}),
    )
}
#[cfg(not(all(feature = "bluetooth", target_os = "linux")))]
fn bluetooth_devices() -> Result<serde_json::Value, machine::MachineError> {
    Err(machine::MachineError::Config(
        "Bluetooth LE requires a Linux build with Bluetooth enabled".into(),
    ))
}

#[cfg(all(feature = "bluetooth", target_os = "linux"))]
fn send_bluetooth(
    caps: &machine::Capabilities,
    target: &BluetoothTarget,
    job: &machine::CutJob,
) -> Result<serde_json::Value, machine::MachineError> {
    if caps.bluetooth != Some(true) {
        return Err(machine::MachineError::Config(
            "Bluetooth support is not documented for this model".into(),
        ));
    }
    let mut transport: Box<dyn machine::Transport> = if target.ble {
        Box::new(machine::BleTransport::open(&target.address)?)
    } else {
        Box::new(machine::BluetoothTransport::open(
            &target.address,
            target.channel,
        )?)
    };
    transport.write(machine::ESC_INIT.as_bytes())?;
    let version = machine::query_version(transport.as_mut())?;
    if machine::DeviceRegistry::builtin().by_firmware(&version) != Some(caps) {
        return Err(machine::MachineError::Protocol(format!(
            "Bluetooth firmware {version:?} does not identify the selected model {}",
            caps.id
        )));
    }
    let mut driver = machine::MachineDriver::new(caps.clone(), transport);
    let report = driver.send(job, &mut machine::NoHooks)?;
    Ok(serde_json::json!({"sent":true,"bytes":report.bytes,"firmware":report.version}))
}
#[cfg(not(all(feature = "bluetooth", target_os = "linux")))]
fn send_bluetooth(
    _: &machine::Capabilities,
    _: &BluetoothTarget,
    _: &machine::CutJob,
) -> Result<serde_json::Value, machine::MachineError> {
    Err(machine::MachineError::Config(
        "Bluetooth requires a Linux build with Bluetooth enabled".into(),
    ))
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
            depth: None,
            tool: None,
            draw: false,
            holder: 1,
            passes: 1,
            mirror: false,
            bluetooth: None,
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
            depth: None,
            tool: None,
            draw: false,
            holder: 1,
            passes: 1,
            mirror: false,
            bluetooth: None,
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
    fn matless_media_and_autoblade_depth_are_validated() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = crate::Engine::new(stencilcraft_doc::Document::default());
        engine.run(crate::Command::ShapeAddRect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        })?;
        engine.run(crate::Command::DocumentSetBoard {
            width_mm: 25.0,
            height_mm: 50.0,
        })?;
        let mut command = HardwareCommand::Cut {
            model: "cameo-4".into(),
            location: "unused".into(),
            mat: machine::Mat::None,
            force: 10,
            speed: 5,
            depth: None,
            tool: None,
            draw: false,
            holder: 1,
            passes: 1,
            mirror: false,
            bluetooth: None,
            send: false,
        };
        assert!(run(&engine.doc, &command).is_err());
        engine.run(crate::Command::DocumentSetBoard {
            width_mm: 50.0,
            height_mm: 50.0,
        })?;
        if let HardwareCommand::Cut { depth, .. } = &mut command {
            *depth = Some(11);
        }
        assert!(run(&engine.doc, &command).is_err());
        if let HardwareCommand::Cut { depth, .. } = &mut command {
            *depth = Some(1);
        }
        assert!(run(&engine.doc, &command).is_ok());
        Ok(())
    }

    #[test]
    fn hardware_settings_validate_tool_and_passes() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = crate::Engine::new(stencilcraft_doc::Document::default());
        engine.run(crate::Command::ShapeAddRect {
            x: 10.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
        })?;
        let mut command = HardwareCommand::Cut {
            model: "cameo-4".into(),
            location: "unused".into(),
            mat: machine::Mat::Cameo12x12,
            force: 10,
            speed: 5,
            depth: None,
            tool: Some(machine::ToolKind::Pen),
            draw: true,
            holder: 1,
            passes: 1,
            mirror: false,
            bluetooth: None,
            send: false,
        };
        let first = execute(&engine.doc, &command)?;
        if let HardwareCommand::Cut { passes, .. } = &mut command {
            *passes = 2;
        }
        let second = execute(&engine.doc, &command)?;
        assert!(second["bytes"].as_u64() > first["bytes"].as_u64());
        if let HardwareCommand::Cut { draw, .. } = &mut command {
            *draw = false;
        }
        assert!(execute(&engine.doc, &command).is_err());
        if let HardwareCommand::Cut { draw, passes, .. } = &mut command {
            *draw = true;
            *passes = 11;
        }
        assert!(execute(&engine.doc, &command).is_err());
        Ok(())
    }

    #[test]
    fn bluetooth_command_is_serializable_without_sending() -> Result<(), Box<dyn std::error::Error>>
    {
        let command: HardwareCommand = serde_json::from_str(
            r#"{"command":"hardware.cut","params":{"model":"cameo-4","location":"","mat":"none","force":10,"speed":5,"bluetooth":{"address":"00:11:22:33:44:55","ble":true}}}"#,
        )?;
        assert!(matches!(
            command,
            HardwareCommand::Cut {
                send: false,
                bluetooth: Some(BluetoothTarget {
                    channel: 1,
                    ble: true,
                    ..
                }),
                ..
            }
        ));
        let discover: HardwareCommand =
            serde_json::from_str(r#"{"command":"hardware.bluetooth_devices"}"#)?;
        assert!(matches!(discover, HardwareCommand::BluetoothDevices));
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
