//! Golden dry-run transcripts: one file per registry machine under
//! `tests/golden/<id>.txt`. Regenerate with `UPDATE_GOLDEN=1 cargo test -p
//! stencilcraft-machine --test golden`, then review the diff by hand.

use stencilcraft_machine::{
    Capabilities, CutSettings, DeviceRegistry, Media, PlotOptions, dry_run,
};

fn design() -> Vec<Vec<(f64, f64)>> {
    vec![
        vec![
            (5.0, 5.0),
            (15.0, 5.0),
            (15.0, 15.0),
            (5.0, 15.0),
            (5.0, 5.0),
        ],
        vec![(20.0, 5.0), (30.0, 15.0), (20.0, 15.0)],
    ]
}

fn transcript(caps: &Capabilities) -> String {
    let media = Media::by_id(102).unwrap();
    let mut settings = CutSettings::for_caps(caps, &media);
    if caps.cut_width_mm.is_none() {
        settings.media_size_mm = Some((300.0, 300.0));
    }
    let header = format!(
        "# {} ({}) protocol={}\n",
        caps.name,
        caps.id,
        caps.protocol.id()
    );
    let job = settings.to_job(&design(), &PlotOptions::default(), false);
    match dry_run(caps, &job) {
        Ok(p) => header + &p.transcript().join("\n") + "\n",
        Err(e) => header + &format!("error: {e}\n"),
    }
}

#[test]
fn every_machine_matches_its_golden_transcript() {
    assert!(DeviceRegistry::builtin_error().is_none());
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut mismatches = Vec::new();
    for caps in DeviceRegistry::builtin().machines() {
        let path = dir.join(format!("{}.txt", caps.id));
        let actual = transcript(caps);
        if update {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_default();
        if expected != actual {
            mismatches.push(format!("{}:\n{actual}", path.display()));
        }
    }
    assert!(
        mismatches.is_empty(),
        "golden mismatch:\n{}",
        mismatches.join("\n")
    );
}
