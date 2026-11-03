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
                ..Operation::default()
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
                ..Operation::default()
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

fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<(f64, f64)> {
    vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)]
}

fn travel(paths: &[Vec<(f64, f64)>]) -> f64 {
    let mut head = (0.0, 0.0);
    let mut total = 0.0;
    for p in paths {
        let (Some(&a), Some(&b)) = (p.first(), p.last()) else {
            continue;
        };
        total += (a.0 - head.0).hypot(a.1 - head.1);
        head = b;
    }
    total
}

#[test]
fn interior_contours_are_cut_before_their_outline() {
    let outer = rect(0.0, 0.0, 100.0, 100.0);
    let hole = rect(40.0, 40.0, 20.0, 20.0);
    let dot = rect(45.0, 45.0, 5.0, 5.0);
    for order in [CutOrder::None, CutOrder::Speed, CutOrder::Roller] {
        let got = order_paths(vec![outer.clone(), hole.clone(), dot.clone()], order, true);
        assert_eq!(
            got,
            vec![dot.clone(), hole.clone(), outer.clone()],
            "{order:?}"
        );
    }
    // Off + no sort keeps document order.
    let got = order_paths(vec![outer.clone(), hole.clone()], CutOrder::None, false);
    assert_eq!(got, vec![outer, hole]);
}

#[test]
fn roller_order_is_monotonic_in_the_feed_axis() {
    let paths: Vec<_> = [70.0, 10.0, 50.0, 30.0, 90.0]
        .iter()
        .enumerate()
        .map(|(i, &y)| rect(i as f64 * 15.0, y, 10.0, 5.0))
        .collect();
    let got = order_paths(paths, CutOrder::Roller, true);
    let tops: Vec<f64> = got.iter().map(|p| p[0].1).collect();
    assert_eq!(tops, vec![10.0, 30.0, 50.0, 70.0, 90.0]);
}

#[test]
fn speed_order_reduces_travel_versus_no_sort() {
    // A scattered grid in a deliberately bad document order.
    let paths: Vec<_> = (0..25)
        .map(|i| {
            let k = (i * 7) % 25;
            rect(f64::from(k % 5) * 30.0, f64::from(k / 5) * 30.0, 5.0, 5.0)
        })
        .collect();
    let none = order_paths(paths.clone(), CutOrder::None, true);
    let speed = order_paths(paths, CutOrder::Speed, true);
    assert_eq!(none.len(), speed.len());
    assert!(
        travel(&speed) < travel(&none) * 0.5,
        "{} vs {}",
        travel(&speed),
        travel(&none)
    );
}

#[test]
fn closed_overcut_extends_past_the_start_by_its_length() {
    let sq = rect(0.0, 0.0, 10.0, 10.0);
    let ext = extend_closed(&sq, 2.5);
    assert_eq!(ext.len(), sq.len() + 1);
    assert_eq!(ext.last(), Some(&(2.5, 0.0)));
    // Longer than the first segment: wraps onto the next one.
    assert_eq!(extend_closed(&sq, 12.0).last(), Some(&(10.0, 2.0)));
    // Open paths are untouched.
    let open = vec![(0.0, 0.0), (10.0, 0.0)];
    assert_eq!(extend_closed(&open, 2.0), open);
    // Bounded at validation.
    let caps = reg().by_id("cameo-4").unwrap();
    let mut job = vinyl_job(caps);
    for bad in [f64::NAN, -1.0, MAX_CLOSED_OVERCUT_MM + 0.1] {
        job.closed_overcut_mm = bad;
        assert!(dry_run(caps, &job).is_err(), "{bad}");
    }
    job.closed_overcut_mm = 1.0;
    let with = dry_run(caps, &job).unwrap();
    job.closed_overcut_mm = 0.0;
    let without = dry_run(caps, &job).unwrap();
    assert_eq!(with.transcript().len(), without.transcript().len() + 1);
}

#[test]
fn mirror_flips_about_the_media_centre_at_job_build_time() {
    let caps = reg().by_id("cameo-4").unwrap();
    let mut job = vinyl_job(caps);
    job.media_size_mm = Some((200.0, 300.0));
    job.mirror = true;
    let built = apply_job_settings(caps, &job).unwrap();
    let xs: Vec<f64> = built.operations[0].paths[0].iter().map(|p| p.0).collect();
    assert_eq!(xs, vec![200.0, 190.0, 190.0, 200.0, 200.0]);
    // The job itself (and so the document) is untouched.
    assert_eq!(job.operations[0].paths, square_paths());
    dry_run(caps, &job).unwrap();
    // Mirroring with print-and-cut marks is refused.
    job.registration = Some(Registration {
        width_mm: 150.0,
        length_mm: 150.0,
        origin_right_mm: 0.0,
        origin_down_mm: 0.0,
        search: false,
        quad: false,
    });
    assert!(dry_run(caps, &job).is_err());
}

fn path_len(paths: &[Vec<(f64, f64)>]) -> f64 {
    paths
        .iter()
        .flat_map(|p| p.windows(2))
        .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
        .sum()
}

#[test]
fn weld_on_cut_removes_the_doubled_overlap() {
    // Two 20 mm squares overlapping by 10 mm: 160 mm of cut, 100 mm welded.
    let two = vec![rect(0.0, 0.0, 20.0, 20.0), rect(10.0, 0.0, 20.0, 20.0)];
    let welded = weld_paths(two.clone());
    assert_eq!(welded.len(), 1);
    assert!(
        (path_len(&welded) - 100.0).abs() < 1e-6,
        "{}",
        path_len(&welded)
    );
    // A ring keeps its hole; an open path passes through.
    let ring = vec![
        rect(0.0, 0.0, 30.0, 30.0),
        rect(10.0, 10.0, 10.0, 10.0),
        vec![(40.0, 0.0), (50.0, 0.0)],
    ];
    let w = weld_paths(ring.clone());
    assert_eq!(w.len(), 3);
    assert!((path_len(&w) - path_len(&ring)).abs() < 1e-6);
    // Through the job: off by default, one path when on; the job is untouched.
    let caps = reg().by_id("cameo-4").unwrap();
    let mut job = vinyl_job(caps);
    job.operations[0].paths = two.clone();
    let moves = |j: &CutJob| {
        let t = dry_run(caps, j).unwrap().transcript();
        t.iter().filter(|l| l.starts_with('M')).count()
    };
    let without = moves(&job);
    job.weld = true;
    let built = apply_job_settings(caps, &job).unwrap();
    assert_eq!(built.operations[0].paths.len(), 1);
    assert_eq!(moves(&job), without - 1, "one path to cut, not two");
    assert_eq!(job.operations[0].paths, two);
}

fn two_layers() -> Vec<LayerPaths> {
    vec![
        LayerPaths {
            name: "Line #ff0000".into(),
            color: Some("#ff0000".into()),
            paths: vec![rect(0.0, 0.0, 10.0, 10.0)],
        },
        LayerPaths {
            name: "score".into(),
            color: None,
            paths: vec![rect(20.0, 0.0, 10.0, 10.0)],
        },
    ]
}

#[test]
fn cut_by_layer_skips_assigns_holders_and_overrides() {
    let caps = reg().by_id("cameo-4").unwrap();
    // No settings: one operation, as before.
    let mut job = vinyl_job(caps);
    job.split_layers(two_layers(), &[]).unwrap();
    assert_eq!(job.operations.len(), 1);
    assert_eq!(job.operations[0].paths.len(), 2);
    // Skip a layer by colour: its paths drop out.
    let mut job = vinyl_job(caps);
    let skip = LayerSettings::parse("#FF0000:skip").unwrap();
    job.split_layers(two_layers(), &[skip]).unwrap();
    assert_eq!(job.operations.len(), 1);
    assert_eq!(job.operations[0].paths, vec![rect(20.0, 0.0, 10.0, 10.0)]);
    // Tool 2 with its own passes/force/speed reaches the transcript.
    let mut job = vinyl_job(caps);
    let ls = LayerSettings::parse("score:tool=2,passes=2,force=7,speed=3").unwrap();
    job.split_layers(two_layers(), &[ls]).unwrap();
    assert_eq!(job.operations.len(), 2);
    let lines = dry_run(caps, &job).unwrap().transcript();
    let j: Vec<&String> = lines.iter().filter(|l| l.starts_with('J')).collect();
    assert_eq!(j, ["J1", "J2"]);
    assert!(lines.iter().any(|l| l == "FX7,2"), "{lines:?}");
    assert!(lines.iter().any(|l| l == "!3,2"), "{lines:?}");
    // Bad overrides are refused with the layer named; unknown layers too.
    for spec in ["score:passes=50", "score:tool=3", "score:force=999"] {
        let mut job = vinyl_job(caps);
        job.split_layers(two_layers(), &[LayerSettings::parse(spec).unwrap()])
            .unwrap();
        let err = dry_run(caps, &job).unwrap_err().to_string();
        assert!(err.contains("layer score"), "{spec}: {err}");
    }
    let mut job = vinyl_job(caps);
    let missing = LayerSettings::parse("nope:skip").unwrap();
    assert!(job.split_layers(two_layers(), &[missing]).is_err());
    for spec in ["noname", "a:passes=x", "a:colour=1"] {
        assert!(LayerSettings::parse(spec).is_err(), "{spec}");
    }
}
