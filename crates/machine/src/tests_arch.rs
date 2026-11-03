//! Tests for the driver architecture: registry, jobs, protocols, transports.

use super::*;
use crate::protocol::parse_grbl_status;

fn square_paths() -> Vec<Vec<(f64, f64)>> {
    vec![vec![
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (0.0, 10.0),
        (0.0, 0.0),
    ]]
}

fn reg() -> &'static DeviceRegistry {
    assert!(
        DeviceRegistry::builtin_error().is_none(),
        "{:?}",
        DeviceRegistry::builtin_error()
    );
    DeviceRegistry::builtin()
}

fn vinyl_job(caps: &Capabilities) -> CutJob {
    CutSettings::for_caps(caps, &Media::by_id(102).unwrap()).to_job(
        &square_paths(),
        &PlotOptions::default(),
        false,
    )
}

#[test]
fn builtin_registry_parses_and_covers_every_model() {
    let r = reg();
    assert!(r.machines().len() >= 40);
    for m in Model::ALL {
        let caps = m.caps().expect("every Model has a registry row");
        assert_eq!(Model::by_id(&caps.id), Some(*m));
    }
    let pro = r.by_id("cameo-4-pro").unwrap();
    assert_eq!(pro.usb_ids(), Some((0x0b4d, 0x1139)));
    assert_eq!(pro.force.as_ref().map(|f| (f.min, f.max)), Some((1, 33)));
    assert!(pro.roll_feed && pro.overcut && pro.multi_pass);
    assert_eq!(
        r.by_firmware("CAMEO 4 PLUS V1.0").map(|c| c.id.as_str()),
        Some("cameo-4-plus")
    );
    assert_eq!(
        r.by_usb(0x3844, 0x0001).map(|c| c.id.as_str()),
        Some("cameo-5-alpha")
    );
}

#[test]
fn cricut_slots_exist_but_never_encode() {
    let r = reg();
    for id in [
        "cricut-joy",
        "cricut-explore-3",
        "cricut-maker-3",
        "cricut-venture",
    ] {
        let caps = r.by_id(id).unwrap();
        assert_eq!(caps.protocol, ProtocolId::Unsupported);
        assert!(!caps.encodable());
        let job = vinyl_job(caps);
        let err = dry_run(caps, &job).unwrap_err();
        assert!(err.to_string().contains("not implemented"), "{err}");
        let mut d = MachineDriver::new(caps.clone(), Box::new(MockTransport::default()));
        assert!(d.send(&job, &mut NoHooks).is_err());
    }
}

#[test]
fn user_tables_add_and_override_machines() {
    let extra = r#"
[[machine]]
id = "my-roland"
name = "Roland GX-24 (HP-GL)"
vendor = "Roland"
protocol = "hpgl"
cut_width_mm = 584.0
tools = ["drag-blade"]

[[machine]]
id = "cameo-4"
name = "CAMEO 4 (my calibrated limits)"
protocol = "silhouette-cameo4"
cut_width_mm = 300.0
cut_length_mm = 3000.0
tools = ["autoblade"]
force = { min = 1, max = 20, unit = "steps" }
"#;
    let r = reg().with_overrides(extra).unwrap();
    let roland = r.by_id("my-roland").unwrap();
    let job = CutJob {
        operations: vec![Operation {
            holder: 1,
            paths: square_paths(),
            ..Operation::default()
        }],
        ..CutJob::default()
    };
    let p = dry_run(roland, &job).unwrap();
    assert_eq!(p.transcript().first().map(String::as_str), Some("IN;"));
    assert_eq!(
        r.by_id("cameo-4")
            .and_then(|c| c.force.as_ref())
            .map(|f| f.max),
        Some(20)
    );
    // Bad tables are errors, not panics.
    assert!(reg().with_overrides("[[machine]]\nid = \"x\"").is_err());
    assert!(
        DeviceRegistry::from_toml(
            "[[machine]]\nid=\"a\"\nname=\"a\"\nprotocol=\"hpgl\"\nusb=\"zz\""
        )
        .is_err()
    );
    assert!(
        DeviceRegistry::from_toml("[[machine]]\nid=\"a\"\nname=\"a\"\nprotocol=\"warp-drive\"")
            .is_err()
    );
}

#[test]
fn multi_operation_jobs_retool_between_operations() {
    let caps = reg().by_id("cameo-4").unwrap();
    let job = CutJob {
        operations: vec![
            Operation {
                kind: OpKind::Draw,
                tool: Some(ToolKind::Pen),
                holder: 2,
                paths: square_paths(),
                settings: OpSettings {
                    force: Some(10),
                    speed: Some(10),
                    ..OpSettings::default()
                },
            },
            Operation {
                kind: OpKind::Cut,
                tool: Some(ToolKind::AutoBlade),
                holder: 1,
                paths: square_paths(),
                settings: OpSettings {
                    force: Some(20),
                    speed: Some(5),
                    depth: Some(3),
                    passes: 2,
                    ..OpSettings::default()
                },
            },
        ],
        mat: Mat::Cameo12x12,
        ..CutJob::default()
    };
    let lines = dry_run(caps, &job).unwrap().transcript();
    let j: Vec<&String> = lines.iter().filter(|l| l.starts_with('J')).collect();
    assert_eq!(j, ["J2", "J1"]);
    assert_eq!(lines.iter().filter(|l| l.starts_with("TG")).count(), 1);
    assert!(
        lines.iter().any(|l| l == "FC0,1,2"),
        "pen gets no blade offset"
    );
    assert!(lines.iter().any(|l| l == "TF3,1"));
    assert_eq!(lines.iter().filter(|l| l.as_str() == "M0,0").count(), 3);
    let mut bad = job.clone();
    bad.operations[1].settings.force = Some(34);
    assert!(dry_run(caps, &bad).is_err());
}

#[test]
fn transport_specs_parse() {
    assert_eq!(
        TransportSpec::parse("usb").unwrap(),
        TransportSpec::Usb(None)
    );
    assert_eq!(
        TransportSpec::parse("usb:1-4").unwrap(),
        TransportSpec::Usb(Some("1-4".into()))
    );
    assert_eq!(
        TransportSpec::parse("/dev/ttyACM0@115200").unwrap(),
        TransportSpec::Serial {
            path: "/dev/ttyACM0".into(),
            baud: 115200
        }
    );
    assert_eq!(
        TransportSpec::parse("serial:COM3").unwrap(),
        TransportSpec::Serial {
            path: "COM3".into(),
            baud: 9600
        }
    );
    assert_eq!(
        TransportSpec::parse("tcp:10.0.0.5:9100").unwrap(),
        TransportSpec::Tcp("10.0.0.5:9100".into())
    );
    assert!(TransportSpec::parse("tcp:nohost").is_err());
    assert!(TransportSpec::parse("file:").is_err());
    assert!(TransportSpec::parse("serial:/dev/x@fast").is_err());
}

#[test]
fn spool_transport_skips_the_handshake() {
    let path = std::env::temp_dir().join(format!("stencilcraft-spool-{}.gpgl", std::process::id()));
    let caps = reg().by_id("cameo-4-pro").unwrap();
    let job = vinyl_job(caps);
    let t = TransportSpec::File(path.display().to_string())
        .open()
        .unwrap();
    let mut d = MachineDriver::new(caps.clone(), t);
    let report = d.send(&job, &mut NoHooks).unwrap();
    d.close().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(bytes.len(), report.bytes);
    assert!(bytes.starts_with(b"TG9\x03"), "no ESC init on a spool");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn driver_reports_events_and_status() {
    struct Log(Vec<DriverEvent>);
    impl SessionHooks for Log {
        fn event(&mut self, e: DriverEvent) {
            self.0.push(e);
        }
    }
    let caps = reg().by_id("cameo-4-pro").unwrap();
    let job = vinyl_job(caps);
    let mock = MockTransport::with_replies(&[
        b"CAMEO 4 PRO V1.02\x03",
        b"    1\x03",
        b"    0\x03",
        b"    2\x03",
    ]);
    let mut d = MachineDriver::new(caps.clone(), Box::new(mock));
    let mut log = Log(Vec::new());
    d.send(&job, &mut log).unwrap();
    assert_eq!(
        log.0.first(),
        Some(&DriverEvent::Identified("CAMEO 4 PRO V1.02".into()))
    );
    assert!(log.0.contains(&DriverEvent::Status(MachineStatus::Busy)));
    assert_eq!(log.0.last(), Some(&DriverEvent::Done));
    assert_eq!(d.status().unwrap(), MachineStatus::MediaUnloaded);
    // GP-GL has no documented pause.
    assert!(matches!(d.pause(), Err(MachineError::Unsupported(_))));
}

#[test]
fn gcode_protocol_encodes_and_controls_grbl() {
    let caps = reg().by_id("grbl-laser").unwrap();
    let job = CutJob {
        operations: vec![Operation {
            tool: Some(ToolKind::Laser),
            holder: 1,
            paths: vec![vec![(0.0, 0.0), (10.0, 0.0), (10.0, 5.0)]],
            settings: OpSettings {
                force: Some(800),
                speed: Some(1200),
                ..OpSettings::default()
            },
            ..Operation::default()
        }],
        media_size_mm: Some((400.0, 400.0)),
        ..CutJob::default()
    };
    assert_eq!(
        dry_run(caps, &job).unwrap().transcript(),
        vec![
            "G21",
            "G90",
            "G17",
            "M5",
            "G0 X0 Y5",
            "M4 S800",
            "G1 X10 Y5 F1200",
            "G1 X10 Y0 F1200",
            "M5",
            "G0 X0 Y0"
        ]
    );
    let mut over = job.clone();
    over.operations[0].settings.force = Some(1001);
    assert!(dry_run(caps, &over).is_err());
    // Send-response streaming: every line acked; an error reply stops with M5.
    let acks: Vec<&[u8]> = vec![b"ok\r\n"; 10];
    let mut d = MachineDriver::new(caps.clone(), Box::new(MockTransport::with_replies(&acks)));
    d.send(&job, &mut NoHooks).unwrap();
    let mut d = MachineDriver::new(
        caps.clone(),
        Box::new(MockTransport::with_replies(&[b"ok\r\n", b"error:22\r\n"])),
    );
    let err = d.send(&job, &mut NoHooks).unwrap_err();
    assert!(err.to_string().contains("error:22"));
    assert_eq!(
        parse_grbl_status("<Hold:0|MPos:0,0,0>"),
        MachineStatus::Paused
    );
    assert_eq!(parse_grbl_status("<Idle|MPos:0,0,0>"), MachineStatus::Ready);
    let mut mock = MockTransport::default();
    let p = protocol_for(ProtocolId::Gcode);
    p.pause(&mut mock).unwrap();
    p.resume(&mut mock).unwrap();
    p.cancel(&mut mock).unwrap();
    assert_eq!(mock.written, b"!~\x18".to_vec());
}

#[test]
fn probing_identifies_a_machine_over_any_transport() {
    let mut mock = MockTransport::with_replies(&[b"CAMEO 5 ALPHA PLUS V1.0\x03"]);
    let found = reg().probe(&mut mock).unwrap();
    assert_eq!(found.map(|c| c.id.as_str()), Some("cameo-5-alpha-plus"));
    assert_eq!(mock.written, b"\x1b\x04FG\x03".to_vec());
}
