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
    /// Ask the USB cutter at `location` whether it is ready, busy or without media, and for its
    /// firmware version. Nothing moves.
    #[serde(rename = "hardware.status")]
    Status { location: String },
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
        /// Path order: `roller` (default), `speed` or `none` (document order).
        #[serde(default)]
        order: Option<String>,
        /// Cut contours inside a closed path before it (default on).
        #[serde(default = "yes")]
        interior_first: bool,
        /// Extend each closed contour past its start (mm; 0 = off).
        #[serde(default)]
        overcut_mm: f64,
        /// Unite overlapping closed contours before cutting.
        #[serde(default)]
        weld: bool,
        /// Roller track enhancing.
        #[serde(default)]
        track_enhancing: bool,
        /// Bytes per write to the machine (Silhouette's packet size); omitted is Auto, 1000.
        #[serde(default)]
        packet_bytes: Option<usize>,
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

fn yes() -> bool {
    true
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
    if let HardwareCommand::Status { location } = command {
        return usb_status(location);
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
        order,
        interior_first,
        overcut_mm,
        weld,
        track_enhancing,
        packet_bytes,
    } = command
    else {
        return devices();
    };
    let order = match order.as_deref() {
        None => machine::CutOrder::default(),
        Some(name) => machine::CutOrder::parse(name)
            .ok_or_else(|| machine::MachineError::Config(format!("Unknown cut order {name}")))?,
    };
    if !(overcut_mm.is_finite() && (0.0..=10.0).contains(overcut_mm)) {
        return Err(machine::MachineError::Config(
            "Overcut must be 0 to 10 mm".into(),
        ));
    }
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
        order,
        interior_first: *interior_first,
        closed_overcut_mm: *overcut_mm,
        weld: *weld,
        track_enhancing: *track_enhancing,
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
        send_usb(
            caps,
            location,
            &job,
            packet_bytes.unwrap_or(machine::AUTO_PACKET_BYTES),
        )
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
/// A device status as the Send tab shows it.
pub fn status_label(status: &machine::DeviceStatus) -> String {
    match status {
        machine::DeviceStatus::Ready => "ready".to_string(),
        machine::DeviceStatus::Moving => "busy".to_string(),
        machine::DeviceStatus::Unloaded => "no media loaded".to_string(),
        machine::DeviceStatus::Unknown(byte) => format!("unknown (status byte {byte})"),
    }
}

#[cfg(all(feature = "usb", not(target_arch = "wasm32")))]
fn usb_status(location: &str) -> Result<serde_json::Value, machine::MachineError> {
    let mut transport = machine::UsbTransport::open(Some(location))?;
    use machine::Transport as _;
    transport.write(machine::ESC_INIT.as_bytes())?;
    let status = machine::query_status(&mut transport)?;
    let firmware = machine::query_version(&mut transport).ok();
    Ok(serde_json::json!({"status": status_label(&status), "firmware": firmware}))
}
#[cfg(not(all(feature = "usb", not(target_arch = "wasm32"))))]
fn usb_status(_: &str) -> Result<serde_json::Value, machine::MachineError> {
    devices()
}

#[cfg(all(feature = "usb", not(target_arch = "wasm32")))]
fn send_usb(
    caps: &machine::Capabilities,
    location: &str,
    job: &machine::CutJob,
    packet: usize,
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
    let transport =
        machine::PacketTransport::new(machine::UsbTransport::open(Some(location))?, packet);
    let mut driver = machine::MachineDriver::new(caps.clone(), Box::new(transport));
    let report = driver.send(job, &mut machine::NoHooks)?;
    Ok(serde_json::json!({"sent":true,"bytes":report.bytes,"firmware":report.version}))
}
#[cfg(not(all(feature = "usb", not(target_arch = "wasm32"))))]
fn send_usb(
    _: &machine::Capabilities,
    _: &str,
    _: &machine::CutJob,
    _: usize,
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

    #[test]
    fn statuses_read_as_words_and_the_command_round_trips() {
        assert_eq!(status_label(&machine::DeviceStatus::Ready), "ready");
        assert_eq!(status_label(&machine::DeviceStatus::Moving), "busy");
        assert_eq!(
            status_label(&machine::DeviceStatus::Unloaded),
            "no media loaded"
        );
        assert!(status_label(&machine::DeviceStatus::Unknown(9)).contains('9'));
        let command = HardwareCommand::Status {
            location: "1-2".into(),
        };
        let json = serde_json::to_string(&command).unwrap();
        assert!(json.contains("hardware.status"), "{json}");
        assert_eq!(
            serde_json::from_str::<HardwareCommand>(&json).unwrap(),
            command
        );
    }

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
            order: None,
            interior_first: true,
            overcut_mm: 0.0,
            weld: false,
            track_enhancing: false,
            packet_bytes: None,
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
            order: None,
            interior_first: true,
            overcut_mm: 0.0,
            weld: false,
            track_enhancing: false,
            packet_bytes: None,
        };
        assert!(run(&engine.doc, &command).is_ok());
        if let HardwareCommand::Cut { force, .. } = &mut command {
            *force = 1000;
        }
        assert!(run(&engine.doc, &command).is_err());
        // The advanced job settings reach the job, and bad values are refused.
        if let HardwareCommand::Cut {
            force,
            order,
            overcut_mm,
            weld,
            ..
        } = &mut command
        {
            *force = 10;
            *order = Some("speed".into());
            *overcut_mm = 0.5;
            *weld = true;
        }
        assert!(run(&engine.doc, &command).is_ok());
        if let HardwareCommand::Cut { order, .. } = &mut command {
            *order = Some("sideways".into());
        }
        assert!(run(&engine.doc, &command).is_err());
        if let HardwareCommand::Cut {
            order, overcut_mm, ..
        } = &mut command
        {
            *order = None;
            *overcut_mm = 20.0;
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
            order: None,
            interior_first: true,
            overcut_mm: 0.0,
            weld: false,
            track_enhancing: false,
            packet_bytes: None,
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
            order: None,
            interior_first: true,
            overcut_mm: 0.0,
            weld: false,
            track_enhancing: false,
            packet_bytes: None,
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
