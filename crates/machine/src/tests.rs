use super::*;
use crate::job::overcut_path;

fn square_paths() -> Vec<Vec<(f64, f64)>> {
    // A 10 mm square at the origin (x right, y down).
    vec![vec![
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (0.0, 10.0),
        (0.0, 0.0),
    ]]
}

fn vinyl_pro() -> (Model, CutSettings) {
    let media = Media::by_id(102).expect("vinyl preset");
    (Model::Cameo4Pro, CutSettings::from_media(&media))
}

fn ascii(cmds: &[Command]) -> Vec<String> {
    cmds.iter().map(Command::to_ascii).collect()
}

fn limit_err(r: Result<CutProgram, MachineError>) -> String {
    match r {
        Err(MachineError::Limit(m)) => m,
        other => panic!("expected a limit error, got {other:?}"),
    }
}

#[test]
fn unit_conversions() {
    assert_eq!(su(0.05), 1);
    assert_eq!(su(0.9), 18); // stock blade diameter
    assert_eq!(su(600.0), 12_000);
    assert_eq!(su(3000.0), 60_000);
    assert_eq!(18.0 / SU_PER_MM, 0.9);
    assert_eq!((24.0 * SU_PER_INCH) as i64, 12_192);
}

#[test]
fn model_from_version_matches_specific_names_first() {
    assert_eq!(
        Model::from_version("CAMEO 4 PRO V1.02"),
        Some(Model::Cameo4Pro)
    );
    assert_eq!(Model::from_version("CAMEO 4 PLUS"), Some(Model::Cameo4Plus));
    assert_eq!(Model::from_version("CAMEO 4 V0.99"), Some(Model::Cameo4));
    assert_eq!(
        Model::from_version("PORTRAIT4 V1.1"),
        Some(Model::Portrait4)
    );
    assert_eq!(
        Model::from_version("CAMEO 5 ALPHA PLUS V1.0"),
        Some(Model::Cameo5AlphaPlus)
    );
    assert_eq!(
        Model::from_version("CAMEO 5 ALPHA V1.02"),
        Some(Model::Cameo5Alpha)
    );
    assert_eq!(Model::from_version("UNKNOWN PLOTTER"), None);
}

#[test]
fn media_table_lookups() {
    assert_eq!(Media::by_id(102).expect("vinyl").name, "Vinyl Sticker");
    assert_eq!(Media::by_id(102).expect("vinyl").pressure, Some(10));
    assert_eq!(Media::by_id(102).expect("vinyl").speed, Some(5));
    assert!(Media::by_id(300).expect("custom").pressure.is_none());
    assert_eq!(Media::by_name("vinyl sticker").expect("vinyl").id, 102);
    assert!(Media::by_name("unobtanium").is_none());
}

#[test]
fn setup_matches_cameo4_sequence() {
    let (model, settings) = vinyl_pro();
    let expected = vec![
        "TG9",
        "FN0",
        "TB50,0",
        "\\0,0",
        "Z60000,12000",
        "J1",
        "FX10,1",
        "TJ0",
        "!5,1",
        "FC0,1,1",
        "FE0,1",
        "FF1,0,1",
        "FF1,1,1",
        "FX10,1",
        "TJ3",
        "FC18,1,1",
    ];
    assert_eq!(ascii(&encode_setup(model, &settings)), expected);
}

#[test]
fn single_holder_cameo4_family_skips_tool_select() {
    let media = Media::by_id(102).expect("vinyl");
    let settings = CutSettings::for_model(Model::Portrait4, &media);
    let cmds = ascii(&encode_setup(Model::Portrait4, &settings));
    assert!(!cmds.iter().any(|c| c.starts_with('J')));
    assert_eq!(cmds.first().map(String::as_str), Some("TG11"));
}

#[test]
fn cameo3_and_legacy_dialects() {
    let media = Media::by_id(102).expect("vinyl");
    let s3 = CutSettings::for_model(Model::Cameo3, &media);
    assert_eq!(
        ascii(&encode_setup(Model::Cameo3, &s3)),
        vec![
            "TG2",
            "FN0",
            "TB50,0",
            "\\0,0",
            "Z60000,6096",
            "J1",
            "!5,1",
            "FX10,1",
            "FE0,1",
            "FF1,0,1",
            "FF1,1,1",
            "FC0,1,1",
            "FC18,1,1",
        ]
    );
    let sl = CutSettings::for_model(Model::Cameo2, &media);
    assert_eq!(
        ascii(&encode_setup(Model::Cameo2, &sl)),
        vec![
            "FW102",
            "!5",
            "FX10",
            "FC18",
            "FY1",
            "FN0",
            "TB50,0",
            "\\0,0",
            "Z60000,6080",
            "L0",
            "FE0,0",
            "FF0,0,0",
        ]
    );
}

#[test]
fn autoblade_depth_and_track_enhancing() {
    let (model, mut settings) = vinyl_pro();
    settings.tool = Some(ToolKind::AutoBlade);
    settings.depth = Some(4);
    settings.track_enhancing = true;
    let cmds = ascii(&encode_setup(model, &settings));
    assert_eq!(&cmds[cmds.len() - 2..], &["TF4,1", "FY0"]);
    // Depth is never sent for other tools.
    settings.tool = Some(ToolKind::RatchetBlade);
    assert!(
        !ascii(&encode_setup(model, &settings))
            .iter()
            .any(|c| c.starts_with("TF"))
    );
}

#[test]
fn plot_moves_and_draws_swapped_axes() {
    let (cmds, bb) = encode_plot(&square_paths(), &PlotOptions::default(), false);
    assert_eq!(
        ascii(&cmds),
        vec![
            "M0,0", "D0,200", "D200,200", "D200,0", "D0,0", "M200,0", "SO0"
        ]
    );
    assert_eq!(bb, Some((0.0, 0.0, 10.0, 10.0)));
}

#[test]
fn bbox_only_draws_a_rectangle() {
    let paths = vec![vec![(2.0, 1.0), (6.0, 5.0)]];
    let (cmds, _) = encode_plot(&paths, &PlotOptions::default(), true);
    assert_eq!(
        ascii(&cmds),
        vec![
            "M20,40", "D20,120", "D100,120", "D100,40", "D20,40", "M100,0", "SO0"
        ]
    );
}

#[test]
fn delimit_joins_with_etx() {
    let stream = delimit(&[Command::Home, Command::Origin]);
    assert_eq!(stream, b"H\x03SO0\x03".to_vec());
}

#[test]
fn full_program_starts_and_ends_correctly() {
    let (model, settings) = vinyl_pro();
    let program = encode_job(
        model,
        &settings,
        &square_paths(),
        &PlotOptions::default(),
        false,
    )
    .expect("in range");
    assert_eq!(
        program.commands.first(),
        Some(&Command::SelectMat(Mat::CameoPro24x24))
    );
    assert_eq!(program.commands.last(), Some(&Command::Origin));
    assert_eq!(program.bbox_mm, Some((0.0, 0.0, 10.0, 10.0)));
    assert_eq!(program.framing, Framing::Etx);
}

#[test]
fn recording_transport_captures_writes() {
    let mut transport = RecordingTransport::new();
    let program = CutProgram {
        commands: vec![Command::Home],
        ..CutProgram::default()
    };
    send_program(&mut transport, &program).expect("write");
    assert_eq!(transport.bytes, b"H\x03".to_vec());
    assert_eq!(transport.transcript(), "H\n");
    let mut buf = [0u8; 16];
    assert_eq!(transport.read(&mut buf).expect("read"), 0);
}

#[test]
fn parse_status_decodes_known_replies() {
    assert_eq!(parse_status(b"0"), DeviceStatus::Ready);
    assert_eq!(parse_status(b"1"), DeviceStatus::Moving);
    assert_eq!(parse_status(b"2"), DeviceStatus::Unloaded);
    assert_eq!(parse_status(b"    0\x03"), DeviceStatus::Ready);
    assert_eq!(parse_status(b"9"), DeviceStatus::Unknown(b'9'));
    assert_eq!(parse_status(b""), DeviceStatus::Unknown(255));
}

#[test]
fn tool_setup_reply_parses() {
    assert_eq!(
        parse_tool_setup(b"2,7\x03"),
        vec![Some(ToolKind::AutoBlade), Some(ToolKind::Pen)]
    );
    assert_eq!(parse_tool_setup(b"9\x03"), vec![None]);
    assert!(parse_tool_setup(b"").is_empty());
}

#[test]
fn paths_close_closed_polylines() {
    let pl = Vec::from([stencilcraft_geom::Polyline {
        points: Vec::from([Point::new(0.0, 0.0), Point::new(5.0, 0.0)]),
        closed: true,
    }]);
    assert_eq!(
        paths_from_polylines(&pl),
        vec![vec![(0.0, 0.0), (5.0, 0.0), (0.0, 0.0)]]
    );
}

#[test]
fn preview_encoding_clamps_to_model_limits() {
    let (model, mut settings) = vinyl_pro();
    settings.pressure = Some(99);
    settings.speed = Some(199);
    let program = encode_cut(
        model,
        &settings,
        &square_paths(),
        &PlotOptions::default(),
        false,
    );
    let lines = program.transcript();
    let pressures: Vec<&String> = lines.iter().filter(|s| s.starts_with("FX")).collect();
    assert_eq!(pressures, vec!["FX33,1", "FX33,1"]);
    assert!(lines.iter().any(|c| c == "!30,1"));
}

#[test]
fn checked_encoding_rejects_out_of_range_values() {
    let paths = square_paths();
    let opts = PlotOptions::default();
    let (model, base) = vinyl_pro();

    let mut s = base.clone();
    s.pressure = Some(34);
    assert!(
        limit_err(encode_job(model, &s, &paths, &opts, false)).contains("force 34 outside 1..=33")
    );
    let mut s = base.clone();
    s.speed = Some(31);
    assert!(limit_err(encode_job(model, &s, &paths, &opts, false)).contains("speed 31"));
    let mut s = base.clone();
    s.pressure = Some(0);
    assert!(encode_job(model, &s, &paths, &opts, false).is_err());
    // Cameo 3 tops out at speed 10; the Alpha accepts force 40.
    let media = Media::by_id(102).expect("vinyl");
    let mut s = CutSettings::for_model(Model::Cameo3, &media);
    s.speed = Some(11);
    assert!(encode_job(Model::Cameo3, &s, &paths, &opts, false).is_err());
    let mut s = CutSettings::for_model(Model::Cameo5Alpha, &media);
    s.pressure = Some(40);
    assert!(encode_job(Model::Cameo5Alpha, &s, &paths, &opts, false).is_ok());
    s.pressure = Some(41);
    assert!(encode_job(Model::Cameo5Alpha, &s, &paths, &opts, false).is_err());

    let mut s = base.clone();
    s.toolholder = 3;
    assert!(limit_err(encode_job(model, &s, &paths, &opts, false)).contains("tool holder 3"));
    let mut s = CutSettings::for_model(Model::Portrait3, &media);
    s.toolholder = 2;
    assert!(encode_job(Model::Portrait3, &s, &paths, &opts, false).is_err());

    let mut s = base.clone();
    s.tool = Some(ToolKind::AutoBlade);
    s.depth = Some(11);
    assert!(encode_job(model, &s, &paths, &opts, false).is_err());
    s.depth = Some(5);
    s.toolholder = 2;
    assert!(limit_err(encode_job(model, &s, &paths, &opts, false)).contains("holder 1"));

    let mut s = base.clone();
    s.tool = Some(ToolKind::Emboss);
    assert!(limit_err(encode_job(model, &s, &paths, &opts, false)).contains("emboss"));
    let mut s = base.clone();
    s.passes = 0;
    assert!(encode_job(model, &s, &paths, &opts, false).is_err());
    s.passes = 11;
    assert!(encode_job(model, &s, &paths, &opts, false).is_err());
    let mut s = base.clone();
    s.overcut_mm = f64::NAN;
    assert!(encode_job(model, &s, &paths, &opts, false).is_err());
    let mut s = base.clone();
    s.mat = Mat::CameoPlus15x15;
    assert!(limit_err(encode_job(model, &s, &paths, &opts, false)).contains("mat 15x15"));
}

#[test]
fn checked_encoding_rejects_out_of_area_designs() {
    let opts = PlotOptions::default();
    let (model, mut s) = vinyl_pro();
    // 24x24 in mat = 609.6 mm, but the Pro cuts 600 mm.
    let wide = vec![vec![(0.0, 0.0), (601.0, 0.0)]];
    assert!(limit_err(encode_job(model, &s, &wide, &opts, false)).contains("601.00 mm wide"));
    // The 12x12 mat bounds the length at 304.8 mm.
    s.mat = Mat::Cameo12x12;
    let long = vec![vec![(0.0, 0.0), (0.0, 305.0)]];
    assert!(limit_err(encode_job(model, &s, &long, &opts, false)).contains("long"));
    let negative = vec![vec![(-1.0, 0.0), (5.0, 0.0)]];
    assert!(encode_job(model, &s, &negative, &opts, false).is_err());
    let nan = vec![vec![(f64::NAN, 0.0), (5.0, 0.0)]];
    assert!(encode_job(model, &s, &nan, &opts, false).is_err());
    // Margins count toward the extent.
    let near = vec![vec![(0.0, 0.0), (300.0, 0.0)]];
    let margins = PlotOptions {
        margin_right_mm: 10.0,
        ..PlotOptions::default()
    };
    assert!(encode_job(model, &s, &near, &margins, false).is_err());
}

#[test]
fn undocumented_machines_refuse_instead_of_guessing() {
    let media = Media::by_id(102).expect("vinyl");
    let paths = square_paths();
    let opts = PlotOptions::default();
    // Curio: no public protocol.
    let s = CutSettings::for_model(Model::Curio, &media);
    assert!(limit_err(encode_job(Model::Curio, &s, &paths, &opts, false)).contains("Export SVG"));
    // Generic HP-GL: no known width until the user says what is loaded.
    let mut s = CutSettings::for_model(Model::GenericHpgl, &media);
    assert!(
        limit_err(encode_job(Model::GenericHpgl, &s, &paths, &opts, false)).contains("media size")
    );
    s.media_size_mm = Some((300.0, 1000.0));
    assert!(encode_job(Model::GenericHpgl, &s, &paths, &opts, false).is_ok());
    // Force is panel-only on HP-GL and Graphtec CE.
    s.pressure = Some(10);
    assert!(limit_err(encode_job(Model::GenericHpgl, &s, &paths, &opts, false)).contains("panel"));
    let mut s = CutSettings::for_model(Model::GraphtecCe7000_60, &media);
    assert!(s.pressure.is_none() && s.speed.is_none());
    assert!(encode_job(Model::GraphtecCe7000_60, &s, &paths, &opts, false).is_ok());
    s.speed = Some(5);
    assert!(encode_job(Model::GraphtecCe7000_60, &s, &paths, &opts, false).is_err());
    s.speed = None;
    s.track_enhancing = true;
    assert!(encode_job(Model::GraphtecCe7000_60, &s, &paths, &opts, false).is_err());
}

#[test]
fn passes_repeat_each_path() {
    let (model, mut s) = vinyl_pro();
    s.passes = 3;
    let p = encode_job(model, &s, &square_paths(), &PlotOptions::default(), false).expect("ok");
    let moves = p
        .transcript()
        .iter()
        .filter(|c| c.as_str() == "M0,0")
        .count();
    assert_eq!(moves, 3);
}

#[test]
fn overcut_extends_sharp_corners_only() {
    let square = overcut_path(&square_paths()[0], 1.0);
    // Every square corner gets out-and-back, including the closing corner.
    assert_eq!(
        square,
        vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (11.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (10.0, 11.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (-1.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
            (0.0, -1.0),
            (0.0, 0.0),
        ]
    );
    // A gentle polyline (10° turns) is left alone.
    let gentle = vec![(0.0, 0.0), (10.0, 0.0), (19.85, 1.74)];
    assert_eq!(overcut_path(&gentle, 1.0), gentle);
}

#[test]
fn registration_commands_and_session_wait() {
    let (model, mut s) = vinyl_pro();
    s.registration = Some(Registration {
        width_mm: 180.0,
        length_mm: 230.0,
        origin_right_mm: 15.0,
        origin_down_mm: 20.0,
        search: true,
        quad: false,
    });
    let paths = vec![vec![(20.0, 25.0), (30.0, 25.0)]];
    let p = encode_job(model, &s, &paths, &PlotOptions::default(), false).expect("ok");
    let lines = p.transcript();
    let wait = p.registration_wait.expect("waits");
    assert_eq!(
        lines[wait - 7..wait],
        [
            "TB50,0",
            "TB99",
            "TB52,2",
            "TB51,400",
            "TB53,10",
            "TB55,1",
            "TB123,4600,3600,200,100"
        ]
    );
    // The design is shifted so the first mark is the origin.
    assert_eq!(lines[wait], "M100,100");

    // Quad marks only where documented; manual positioning uses TB23.
    if let Some(r) = s.registration.as_mut() {
        r.quad = true;
    }
    assert!(encode_job(model, &s, &paths, &PlotOptions::default(), false).is_err());
    assert!(
        encode_job(
            Model::CameoProMk2,
            &s,
            &paths,
            &PlotOptions::default(),
            false
        )
        .is_ok()
    );
    if let Some(r) = s.registration.as_mut() {
        r.quad = false;
        r.search = false;
    }
    let p = encode_job(model, &s, &paths, &PlotOptions::default(), false).expect("ok");
    assert!(p.transcript().iter().any(|c| c == "TB23,4600,3600"));

    // Session: init, version, ready, setup+regmarks, wait for "0", plot.
    let mut mock =
        MockTransport::with_replies(&[b"CAMEO 4 PRO V1.02\x03", b"    0\x03", b"    0\x03"]);
    let report = send_job(&mut mock, model, &p, &mut || {}).expect("sent");
    assert_eq!(report.version.as_deref(), Some("CAMEO 4 PRO V1.02"));
    assert!(mock.written.starts_with(b"\x1b\x04FG\x03\x1b\x05"));
    assert!(mock.written.ends_with(b"SO0\x03"));

    // No registration reply: nothing after the marks is sent.
    let mut mock = MockTransport::with_replies(&[b"CAMEO 4 PRO V1.02\x03", b"0\x03"]);
    let err = send_job(&mut mock, model, &p, &mut || {}).expect_err("marks missing");
    assert!(err.to_string().contains("registration marks not found"));
    assert!(!mock.written.ends_with(b"SO0\x03"));
}

#[test]
fn session_refuses_a_different_model_and_unloaded_media() {
    let (model, s) = vinyl_pro();
    let p = encode_job(model, &s, &square_paths(), &PlotOptions::default(), false).expect("ok");
    let mut mock = MockTransport::with_replies(&[b"PORTRAIT4 V1.1\x03"]);
    let err = send_job(&mut mock, model, &p, &mut || {}).expect_err("mismatch");
    assert!(err.to_string().contains("portrait-4"));
    assert_eq!(mock.written, b"\x1b\x04FG\x03".to_vec());
    let mut mock = MockTransport::with_replies(&[b"CAMEO 4 PRO\x03", b"2\x03"]);
    assert!(send_job(&mut mock, model, &p, &mut || {}).is_err());
    let mut polls = 0;
    let mut mock = MockTransport::with_replies(&[b"CAMEO 4 PRO\x03", b"1\x03", b"1\x03", b"0\x03"]);
    send_job(&mut mock, model, &p, &mut || polls += 1).expect("ready after polls");
    assert_eq!(polls, 2);
}

#[test]
fn status_and_tool_queries_through_a_mock() {
    let mut mock = MockTransport::with_replies(&[b"    1\x03", b"2,0\x03", b"CAMEO 5 V1.0\x03"]);
    assert_eq!(
        query_status(&mut mock).expect("status"),
        DeviceStatus::Moving
    );
    assert_eq!(
        query_tools(&mut mock).expect("tools"),
        vec![Some(ToolKind::AutoBlade), Some(ToolKind::Empty)]
    );
    assert_eq!(query_version(&mut mock).expect("version"), "CAMEO 5 V1.0");
    assert_eq!(mock.written, b"\x1b\x05\x1b\x15FG\x03".to_vec());
}

#[test]
fn enumeration_matches_catalogued_cutters() {
    let ids = |v, p, loc: &str| UsbIds {
        vendor_id: v,
        product_id: p,
        location: loc.to_string(),
        product: None,
    };
    let found = match_cutters([
        ids(0x046d, 0xc52b, "1-2"), // a mouse
        ids(0x0b4d, 0x1137, "1-3"),
        ids(0x3844, 0x0002, "2-1"),
        ids(0x0b4d, 0x9999, "2-2"), // Graphtec vendor, unknown product
    ]);
    let got: Vec<(&str, Model)> = found
        .iter()
        .map(|(d, m)| (d.location.as_str(), *m))
        .collect();
    assert_eq!(
        got,
        vec![("1-3", Model::Cameo4), ("2-1", Model::Cameo5AlphaPlus)]
    );
}

#[test]
fn plotter_dialects_frame_correctly() {
    let media = Media::by_id(102).expect("vinyl");
    let mut s = CutSettings::for_model(Model::GenericHpgl, &media);
    s.media_size_mm = Some((300.0, 300.0));
    let p = encode_job(
        Model::GenericHpgl,
        &s,
        &square_paths(),
        &PlotOptions::default(),
        false,
    )
    .expect("hpgl");
    assert_eq!(
        p.transcript(),
        vec![
            "IN;",
            "SP1;",
            "PU0,400;",
            "PD400,400,400,0,0,0,0,400;",
            "PU;",
            "SP0;"
        ]
    );
    assert_eq!(p.framing, Framing::Newline);
    assert!(p.to_stream().starts_with(b"IN;\nSP1;\n"));

    let p = encode_job(
        Model::GenericDmpl,
        &s,
        &square_paths(),
        &PlotOptions::default(),
        false,
    )
    .expect("dmpl");
    assert_eq!(p.transcript()[..3], [";:", "EC1", "U0,394"]);
    assert_eq!(p.transcript().last().map(String::as_str), Some("@"));

    let mut s = CutSettings::for_model(Model::GenericGpgl, &media);
    s.media_size_mm = Some((300.0, 300.0));
    s.speed = Some(4);
    let p = encode_job(
        Model::GenericGpgl,
        &s,
        &square_paths(),
        &PlotOptions::default(),
        false,
    )
    .expect("gpgl");
    assert_eq!(
        p.transcript(),
        vec![
            "!4", "M0,0", "D0,100", "D100,100", "D100,0", "D0,0", "M100,0"
        ]
    );
    assert_eq!(p.to_stream().last(), Some(&ETX));
}

#[test]
fn ids_round_trip() {
    for m in Mat::ALL {
        assert_eq!(Mat::parse(m.id()), Some(*m));
    }
    assert_eq!(ToolKind::parse("AutoBlade"), Some(ToolKind::AutoBlade));
    assert_eq!(ToolKind::parse("empty"), None);
    assert_eq!(Model::by_id("Cameo-4-Pro"), Some(Model::Cameo4Pro));
}
