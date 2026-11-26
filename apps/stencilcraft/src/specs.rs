//! Gherkin spec runner.
//!
//! `.feature` files under `apps/stencilcraft/specs/features/` are parsed here and
//! every scenario is driven through the real UI with the `egui_kittest` harness
//! from [`crate::ui_e2e`]. Steps speak user language; each one maps to a real
//! widget label or to the engine document, never to a private implementation
//! detail. Scenarios tagged `@gap` document UI that does not exist yet: they are
//! skipped by default and become the gap report when `SPEC_INCLUDE_GAPS=1`.
//!
//! ```text
//! cargo test -p stencilcraft --bin stencilcraft specs
//! SPEC_INCLUDE_GAPS=1 cargo test -p stencilcraft --bin stencilcraft specs
//! ```

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use craft_ui::behavior::Selection;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use stencilcraft_engine::ProductionCommand;

use crate::ui_e2e::{
    App, add_nested_fixture, boot, boot_inspector, boot_menu, boot_profiles, boot_workspace, click,
    click_menu, expand_all, has, has_row_containing, raw_click_label, raw_click_row_containing,
};
use crate::*;

#[derive(Debug)]
struct Scenario {
    name: String,
    tags: Vec<String>,
    steps: Vec<String>,
    line: usize,
}

#[derive(Debug)]
struct Feature {
    name: String,
    path: PathBuf,
    scenarios: Vec<Scenario>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Surface {
    Layers,
    Inspector,
    Workspace,
    Profiles,
    Menu,
}

struct World {
    surface: Surface,
    h: Option<App>,
}

impl World {
    fn new() -> Self {
        Self {
            surface: Surface::Layers,
            h: None,
        }
    }

    fn h(&mut self) -> &mut App {
        if self.h.is_none() {
            self.h = Some(match self.surface {
                Surface::Layers => boot(),
                Surface::Inspector => boot_inspector(),
                Surface::Workspace => boot_workspace(),
                Surface::Profiles => boot_profiles(),
                Surface::Menu => boot_menu(),
            });
        }
        self.h.as_mut().expect("harness was just booted")
    }

    fn state(&mut self) -> &StencilApp {
        self.h().state()
    }

    fn state_mut(&mut self) -> &mut StencilApp {
        self.h().state_mut()
    }

    fn run(&mut self) {
        self.h().run();
    }

    fn surface(&mut self, surface: Surface) -> Result<(), String> {
        if self.h.is_some() {
            return Err("the surface must be chosen before the first UI step".into());
        }
        self.surface = surface;
        Ok(())
    }
}

fn parse_dir(dir: &Path) -> Vec<Feature> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "feature"))
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths.into_iter().filter_map(|p| parse_file(&p)).collect()
}

fn parse_file(path: &Path) -> Option<Feature> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut feature = Feature {
        name: path.file_stem()?.to_string_lossy().into_owned(),
        path: path.to_path_buf(),
        scenarios: Vec::new(),
    };
    let mut feature_tags: Vec<String> = Vec::new();
    let mut pending_tags: Vec<String> = Vec::new();
    let mut background: Vec<String> = Vec::new();
    let mut in_background = false;
    let mut current: Option<Scenario> = None;

    let flush = |current: &mut Option<Scenario>, scenarios: &mut Vec<Scenario>| {
        if let Some(s) = current.take() {
            scenarios.push(s);
        }
    };

    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('|') {
            continue;
        }
        if line.starts_with('@') {
            flush(&mut current, &mut feature.scenarios);
            pending_tags = line
                .split_whitespace()
                .map(|t| t.trim_start_matches('@').to_string())
                .collect();
            continue;
        }
        if let Some(rest) = line.strip_prefix("Feature:") {
            flush(&mut current, &mut feature.scenarios);
            feature.name = rest.trim().to_string();
            feature_tags = std::mem::take(&mut pending_tags);
            continue;
        }
        if line.starts_with("Background:") {
            in_background = true;
            continue;
        }
        if let Some(rest) = line
            .strip_prefix("Scenario Outline:")
            .or_else(|| line.strip_prefix("Scenario:"))
        {
            flush(&mut current, &mut feature.scenarios);
            let mut tags = feature_tags.clone();
            tags.extend(std::mem::take(&mut pending_tags));
            current = Some(Scenario {
                name: rest.trim().to_string(),
                tags,
                steps: background.clone(),
                line: idx + 1,
            });
            in_background = false;
            continue;
        }
        if line.starts_with("Examples:") {
            in_background = false;
            continue;
        }
        let step = ["Given ", "When ", "Then ", "And ", "But ", "* "]
            .iter()
            .find_map(|kw| line.strip_prefix(kw));
        if let Some(step) = step {
            if in_background {
                background.push(step.trim().to_string());
            } else if let Some(s) = current.as_mut() {
                s.steps.push(step.trim().to_string());
            }
        }
    }
    flush(&mut current, &mut feature.scenarios);
    Some(feature)
}

fn nums(text: &str) -> Vec<u64> {
    let mut out = Vec::new();
    let mut run = String::new();
    for c in text.chars() {
        if c.is_ascii_digit() {
            run.push(c);
        } else if !run.is_empty() {
            if let Ok(n) = run.parse() {
                out.push(n);
            }
            run.clear();
        }
    }
    if let Ok(n) = run.parse() {
        out.push(n);
    }
    out
}

fn unquote(text: &str) -> &str {
    text.trim().trim_matches('"')
}

fn between<'a>(text: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    text.strip_prefix(prefix)?.strip_suffix(suffix)
}

fn sorted(mut v: Vec<u64>) -> Vec<u64> {
    v.sort_unstable();
    v
}

/// The id and name of the 1-based `n`th operation.
fn op_at(w: &mut World, n: usize) -> Result<(u64, String), String> {
    w.state()
        .engine
        .doc
        .operations
        .get(n.saturating_sub(1))
        .map(|op| (op.id, op.name.clone()))
        .ok_or_else(|| format!("there is no operation {n}"))
}

fn open_op_menu(w: &mut World, name: &str) {
    click(w.h(), &format!("Operation menu {name}"));
}

/// Whether the inspector subtree (rooted at the inspector header) contains
/// `needle` in any accessible label or value. `PropertyRow::word` values are
/// exposed as `value`, and the inspector shares the window with the lists, so
/// the query is scoped to the header's parent.
fn inspector_shows(h: &mut App, needle: &str) -> bool {
    use egui_kittest::kittest::{By, NodeT as _, Queryable as _};
    let Some(header) = h
        .query_all(By::new().predicate(|node| {
            node.role() == egui::accesskit::Role::Header
                && node.label().is_some_and(|l| {
                    l.starts_with("Shape #") || l == "Document" || l.ends_with(" objects")
                })
        }))
        .next()
    else {
        return false;
    };
    let Some(root) = header.parent() else {
        return false;
    };
    root.children_recursive().any(|node| {
        let a = node.accesskit_node();
        a.label().is_some_and(|l| l.contains(needle))
            || a.value().is_some_and(|v| v.contains(needle))
    })
}

fn fail(msg: impl Into<String>) -> Result<(), String> {
    Err(msg.into())
}

fn step(w: &mut World, text: &str) -> Result<(), String> {
    // --- Given -------------------------------------------------------------
    if text == "a nested fixture with groups \"outer\" and \"inner\"" {
        add_nested_fixture(w.state_mut());
        w.run();
        return Ok(());
    }
    if let Some(name) = between(text, "a second operation \"", "\" over all shapes") {
        let members: Vec<u64> = w.state().engine.doc.shapes.iter().map(|s| s.id).collect();
        let cut = operations::OperationPreset::Cut;
        w.state_mut()
            .run_command(Command::Document(DocumentCommand::OperationCreate {
                name: name.to_string(),
                kind: cut.kind(),
                settings: Default::default(),
                color: None,
                icon: Some(cut.icon_name().to_string()),
                members,
            }));
        w.run();
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("an operation \"") {
        if let Some((name, tail)) = rest.split_once("\" over shapes ") {
            let members = nums(tail);
            if members.is_empty() {
                return fail(format!("expected at least one shape id in {tail:?}"));
            }
            let cut = operations::OperationPreset::Cut;
            w.state_mut()
                .run_command(Command::Document(DocumentCommand::OperationCreate {
                    name: name.to_string(),
                    kind: cut.kind(),
                    settings: Default::default(),
                    color: None,
                    icon: Some(cut.icon_name().to_string()),
                    members,
                }));
            w.run();
            return Ok(());
        }
    }
    if let Some(rest) = text.strip_prefix("shape ") {
        if let Some((left, right)) = rest.split_once(" has stroke \"") {
            let id = nums(left).first().copied().ok_or("shape id missing")?;
            let hex = right.strip_suffix('"').ok_or("stroke colour missing")?;
            w.state_mut()
                .run_command(Command::Production(ProductionCommand::SetProduction {
                    ids: vec![id],
                    operation: None,
                    line_color: Some(hex.to_string()),
                    fill_color: None,
                    material: None,
                    send: None,
                    clear: Vec::new(),
                }));
            w.run();
            return Ok(());
        }
        if let Some((left, right)) = rest.split_once(" has fill \"") {
            let id = nums(left).first().copied().ok_or("shape id missing")?;
            let hex = right.strip_suffix('"').ok_or("fill colour missing")?;
            w.state_mut()
                .run_command(Command::Production(ProductionCommand::SetProduction {
                    ids: vec![id],
                    operation: None,
                    line_color: None,
                    fill_color: Some(hex.to_string()),
                    material: None,
                    send: None,
                    clear: Vec::new(),
                }));
            w.run();
            return Ok(());
        }
    }
    if text.starts_with("a document with ") && text.contains("shape") {
        let n = nums(text).first().copied().unwrap_or(0);
        for i in 0..n {
            let x = 5.0 + i as f64 * 15.0;
            w.state_mut().run_command(Command::ShapeAddRect {
                x,
                y: 5.0,
                width: 10.0,
                height: 10.0,
            });
        }
        w.run();
        return Ok(());
    }
    if text == "all groups are expanded" {
        expand_all(w.state_mut());
        w.run();
        return Ok(());
    }
    if text == "the unassigned group is expanded" {
        w.state_mut()
            .open_geo_groups
            .insert(stencilcraft_ui_egui::layers::UNASSIGNED_GROUP);
        w.run();
        return Ok(());
    }
    if let Some(n) = text.strip_prefix("the bulk confirm threshold is ") {
        w.state_mut().prefs.bulk_confirm_threshold = n
            .trim()
            .parse()
            .map_err(|_| format!("the bulk confirm threshold must be a number, got {n:?}"))?;
        w.run();
        return Ok(());
    }
    if let Some(list) = text
        .strip_prefix("shapes ")
        .and_then(|r| r.strip_suffix(" are selected"))
    {
        w.state_mut().set_selection(nums(list));
        w.run();
        return Ok(());
    }
    if let Some(id) = text
        .strip_prefix("shape ")
        .and_then(|r| r.strip_suffix(" is selected"))
    {
        let id = nums(id).first().copied().ok_or("shape id missing")?;
        w.state_mut().set_selection([id]);
        w.run();
        return Ok(());
    }
    if let Some(id) = text
        .strip_prefix("group ")
        .and_then(|r| r.strip_suffix(" is selected in the geometry panel"))
    {
        let id = nums(id).first().copied().ok_or("group id missing")?;
        w.state_mut().geo_selection = Selection::only(GeoKey::Group(id));
        w.run();
        return Ok(());
    }
    if text == "the inspector panel is open" {
        return w.surface(Surface::Inspector);
    }
    if text == "the workspace is open" {
        return w.surface(Surface::Workspace);
    }
    if text == "the profiles panel is open" {
        return w.surface(Surface::Profiles);
    }
    if text == "the menu bar is open" {
        return w.surface(Surface::Menu);
    }

    // --- When --------------------------------------------------------------
    if let Some(rest) = text.strip_prefix("I expand operation ") {
        let n = nums(rest)
            .first()
            .copied()
            .ok_or("operation number missing")? as usize;
        let (_, name) = op_at(w, n)?;
        click(w.h(), &format!("Expand {name}"));
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I remove member #") {
        let (shape, op) = rest
            .split_once(" from operation ")
            .ok_or_else(|| format!("expected `from operation N` in {rest:?}"))?;
        let shape = nums(shape).first().copied().ok_or("shape id missing")?;
        let n = nums(op)
            .first()
            .copied()
            .ok_or("operation number missing")? as usize;
        let (_, name) = op_at(w, n)?;
        click(w.h(), &format!("Remove #{shape} from {name}"));
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I open the colour of operation ") {
        let n = nums(rest)
            .first()
            .copied()
            .ok_or("operation number missing")? as usize;
        let (_, name) = op_at(w, n)?;
        click(w.h(), &format!("Colour {name}"));
        return Ok(());
    }
    if let Some(hex) = between(text, "I set the colour to \"", "\"") {
        w.h().get_by_label("Hex").set_text_value_accesskit(hex);
        w.run();
        return Ok(());
    }
    if text == "I close the colour picker" {
        click(w.h(), "Close");
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I focus operation ") {
        let n = nums(rest)
            .first()
            .copied()
            .ok_or("operation number missing")? as usize;
        let (_, name) = op_at(w, n)?;
        raw_click_row_containing(w.h(), &name);
        return Ok(());
    }
    if text == "I press Ctrl+[" {
        w.h()
            .key_press_modifiers(egui::Modifiers::CTRL, egui::Key::OpenBracket);
        w.run();
        return Ok(());
    }
    if text == "I press Ctrl+]" {
        w.h()
            .key_press_modifiers(egui::Modifiers::CTRL, egui::Key::CloseBracket);
        w.run();
        return Ok(());
    }
    if let Some(id) = text
        .strip_prefix("I remove shape #")
        .and_then(|r| r.strip_suffix(" from the last operation"))
    {
        let id = nums(id).first().copied().ok_or("shape id missing")?;
        let name = w
            .state()
            .engine
            .doc
            .operations
            .last()
            .map(|op| op.name.clone())
            .ok_or("there is no operation to remove membership from")?;
        click(w.h(), &format!("Operation {name} on #{id}"));
        click_menu(w.h(), &format!("Remove #{id} from {name}"));
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I open the settings for operation ") {
        let n = nums(rest)
            .first()
            .copied()
            .ok_or("operation number missing")? as usize;
        let (_, name) = op_at(w, n)?;
        click(w.h(), &format!("Operation settings {name}"));
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I set the power to ") {
        let power = rest
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("power must be a number, got {rest:?}"))?;
        w.h().get_by_label("Power").set_value_accesskit(power);
        w.run();
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I set operation ") {
        let n = nums(rest)
            .first()
            .copied()
            .ok_or("operation number missing")? as usize;
        let kind = rest
            .split_once("to kind \"")
            .and_then(|(_, r)| r.strip_suffix('"'))
            .ok_or_else(|| format!("expected `to kind \"…\"` in {rest:?}"))?;
        let preset = crate::operations::OperationPreset::ALL
            .iter()
            .copied()
            .find(|p| p.kind().kind_name() == kind)
            .ok_or_else(|| format!("unknown operation kind {kind:?}"))?;
        let (_, name) = op_at(w, n)?;
        open_op_menu(w, &name);
        click_menu(w.h(), &format!("Set kind {} for {name}", preset.label()));
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I toggle the lock on operation ") {
        let n = nums(rest)
            .first()
            .copied()
            .ok_or("operation number missing")? as usize;
        let (_, name) = op_at(w, n)?;
        // The visible lock toggle in the operations list; toggle cells need a
        // real pointer press/release, not an accesskit click.
        raw_click_label(w.h(), &format!("Lock {name}"));
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I move operation ") {
        let n = nums(rest)
            .first()
            .copied()
            .ok_or("operation number missing")? as usize;
        let (_, name) = op_at(w, n)?;
        let direction = if rest.ends_with("up") { "up" } else { "down" };
        open_op_menu(w, &name);
        click_menu(w.h(), &format!("Move {direction} {name}"));
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I delete operation ") {
        let n = nums(rest)
            .first()
            .copied()
            .ok_or("operation number missing")? as usize;
        let (_, name) = op_at(w, n)?;
        open_op_menu(w, &name);
        click_menu(w.h(), &format!("Delete {name}"));
        return Ok(());
    }
    if let Some(label) = between(text, "I click the geometry row \"", "\"") {
        raw_click_label(w.h(), label);
        return Ok(());
    }
    if let Some(label) = between(text, "I click the operation row \"", "\"") {
        raw_click_label(w.h(), label);
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I select operation ") {
        let n: usize = rest
            .parse()
            .map_err(|_| format!("not a number: {rest:?}"))?;
        let index = n.checked_sub(1).ok_or("operation numbers start at 1")?;
        if index >= w.state().engine.doc.operations.len() {
            return Err(format!("there is no operation {n}"));
        }
        w.state_mut().op_selection = Selection::only(OpKey::Op(index));
        w.run();
        return Ok(());
    }
    if text == "I click the first operation row" {
        let label = w
            .state()
            .engine
            .doc
            .operations
            .first()
            .map(|op| op.name.clone())
            .ok_or("there is no operation row")?;
        raw_click_row_containing(w.h(), &label);
        return Ok(());
    }
    if let Some(label) = between(text, "I double-click the member row \"", "\"") {
        // A list row's Activate fires on the second press/release pair in one
        // frame batch; a plain accesskit click never reaches it.
        let pos = w.h().get_by_label(label).rect().center();
        for _ in 0..2 {
            w.h().event(egui::Event::PointerMoved(pos));
            for pressed in [true, false] {
                w.h().event(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            w.h().run();
        }
        return Ok(());
    }
    if let Some(n) = text.strip_prefix("I click the palette colour ") {
        raw_click_label(w.h(), &format!("Palette colour {}", n.trim()));
        return Ok(());
    }
    if let Some(label) = between(text, "I choose the appearance value \"", "\"") {
        click(w.h(), label);
        return Ok(());
    }
    if let Some(rest) = text.strip_prefix("I click the \"") {
        if let Some((label, op)) = rest.split_once("\" button for operation ") {
            let n = nums(op)
                .first()
                .copied()
                .ok_or("operation number missing")?;
            click(w.h(), &format!("{label} for operation {n}"));
            return Ok(());
        }
        if let Some(label) = rest.strip_suffix("\" button") {
            click(w.h(), label);
            return Ok(());
        }
    }
    if let Some(label) = between(text, "I open the \"", "\" menu") {
        click(w.h(), label);
        return Ok(());
    }
    if let Some(label) = between(text, "I choose \"", "\"") {
        click_menu(w.h(), label);
        return Ok(());
    }
    if let Some(label) = between(text, "I click \"", "\"") {
        click(w.h(), label);
        return Ok(());
    }

    // --- Then --------------------------------------------------------------
    if let Some(n) = text
        .strip_prefix("the document has ")
        .or_else(|| text.strip_prefix("the document still has "))
    {
        let want = nums(n).first().copied().ok_or("operation count missing")? as usize;
        let got = w.state().engine.doc.operations.len();
        if n.contains("group") {
            let got = w.state().engine.doc.groups.len();
            return if got == want {
                Ok(())
            } else {
                fail(format!("expected {want} groups, got {got}"))
            };
        }
        return if got == want {
            Ok(())
        } else {
            fail(format!("expected {want} operations, got {got}"))
        };
    }
    if let Some(kind) = between(text, "the last operation has kind \"", "\"") {
        let op = w
            .state()
            .engine
            .doc
            .operations
            .last()
            .ok_or("there is no operation")?;
        return if op.kind_name() == kind {
            Ok(())
        } else {
            fail(format!("expected kind {kind:?}, got {:?}", op.kind_name()))
        };
    }
    if let Some(list) = text.strip_prefix("the last operation has members ") {
        let op = w
            .state()
            .engine
            .doc
            .operations
            .last()
            .ok_or("there is no operation")?;
        let want = sorted(nums(list));
        let got = sorted(op.members.clone());
        return if got == want {
            Ok(())
        } else {
            fail(format!("expected members {want:?}, got {got:?}"))
        };
    }
    if let Some(rest) = text.strip_prefix("operation ") {
        if let Some((left, right)) = rest.split_once(" has power ") {
            let idx = nums(left)
                .first()
                .copied()
                .ok_or("operation number missing")? as usize;
            let want = right
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("power must be a number, got {right:?}"))?;
            let op = w
                .state()
                .engine
                .doc
                .operations
                .get(idx.saturating_sub(1))
                .ok_or_else(|| format!("there is no operation {idx}"))?;
            return if (op.settings.power - want).abs() < 0.01 {
                Ok(())
            } else {
                fail(format!(
                    "expected operation {idx} power {want}, got {}",
                    op.settings.power
                ))
            };
        }
        if let Some((left, right)) = rest.split_once(" has kind ") {
            let idx = nums(left)
                .first()
                .copied()
                .ok_or("operation number missing")? as usize;
            let kind = unquote(right);
            let op = w
                .state()
                .engine
                .doc
                .operations
                .get(idx.saturating_sub(1))
                .ok_or_else(|| format!("there is no operation {idx}"))?;
            return if op.kind_name() == kind {
                Ok(())
            } else {
                fail(format!(
                    "expected operation {idx} kind {kind:?}, got {:?}",
                    op.kind_name()
                ))
            };
        }
        if let Some((left, right)) = rest.split_once(" has colour ") {
            let idx = nums(left)
                .first()
                .copied()
                .ok_or("operation number missing")? as usize;
            let want = unquote(right);
            let op = w
                .state()
                .engine
                .doc
                .operations
                .get(idx.saturating_sub(1))
                .ok_or_else(|| format!("there is no operation {idx}"))?;
            return if op.color.as_deref() == Some(want) {
                Ok(())
            } else {
                fail(format!(
                    "expected operation {idx} colour {want:?}, got {:?}",
                    op.color
                ))
            };
        }
        if let Some((left, right)) = rest.split_once(" has members ") {
            let idx = nums(left)
                .first()
                .copied()
                .ok_or("operation number missing")? as usize;
            let op = w
                .state()
                .engine
                .doc
                .operations
                .get(idx.saturating_sub(1))
                .ok_or_else(|| format!("there is no operation {idx}"))?;
            let want = sorted(nums(right));
            let got = sorted(op.members.clone());
            return if got == want {
                Ok(())
            } else {
                fail(format!(
                    "expected operation {idx} members {want:?}, got {got:?}"
                ))
            };
        }
        if let Some(idx) = rest.strip_suffix(" is locked") {
            let idx = nums(idx)
                .first()
                .copied()
                .ok_or("operation number missing")? as usize;
            let op = w
                .state()
                .engine
                .doc
                .operations
                .get(idx.saturating_sub(1))
                .ok_or_else(|| format!("there is no operation {idx}"))?;
            return if op.locked {
                Ok(())
            } else {
                fail(format!("operation {idx} is not locked"))
            };
        }
        if let Some(idx) = rest.strip_suffix(" is visible") {
            let idx = nums(idx)
                .first()
                .copied()
                .ok_or("operation number missing")? as usize;
            let op = w
                .state()
                .engine
                .doc
                .operations
                .get(idx.saturating_sub(1))
                .ok_or_else(|| format!("there is no operation {idx}"))?;
            return if op.visible {
                Ok(())
            } else {
                fail(format!("operation {idx} is not visible"))
            };
        }
    }
    if let Some(rest) = text.strip_prefix("operation \"") {
        if let Some((name, tail)) = rest.split_once("\" has ") {
            let want = nums(tail).first().copied().unwrap_or(0) as usize;
            let op = w
                .state()
                .engine
                .doc
                .operations
                .iter()
                .find(|op| op.name == name)
                .ok_or_else(|| format!("there is no operation named {name:?}"))?;
            return if op.members.len() == want {
                Ok(())
            } else {
                fail(format!(
                    "expected operation {name:?} to have {want} members, got {}",
                    op.members.len()
                ))
            };
        }
        if let Some(name) = rest.strip_suffix("\" is visible") {
            let op = w
                .state()
                .engine
                .doc
                .operations
                .iter()
                .find(|op| op.name == name)
                .ok_or_else(|| format!("there is no operation named {name:?}"))?;
            return if op.visible {
                Ok(())
            } else {
                fail(format!("operation {name:?} is not visible"))
            };
        }
    }
    if let Some(needle) = between(text, "the operations panel shows \"", "\"") {
        return if has_row_containing(w.h(), needle) {
            Ok(())
        } else {
            fail(format!("the operations panel does not show {needle:?}"))
        };
    }
    if let Some(needle) = between(text, "the operations panel does not show \"", "\"") {
        return if has_row_containing(w.h(), needle) {
            fail(format!("the operations panel still shows {needle:?}"))
        } else {
            Ok(())
        };
    }
    if let Some(needle) = between(text, "the geometry panel shows \"", "\"") {
        return if has_row_containing(w.h(), needle) {
            Ok(())
        } else {
            fail(format!("the geometry panel does not show {needle:?}"))
        };
    }
    if let Some(needle) = between(text, "the geometry panel does not show \"", "\"") {
        return if has_row_containing(w.h(), needle) {
            fail(format!("the geometry panel still shows {needle:?}"))
        } else {
            Ok(())
        };
    }
    if let Some(needle) = between(text, "the inspector shows the header \"", "\"") {
        return if has(w.h(), needle) {
            Ok(())
        } else {
            fail(format!("the inspector header is not {needle:?}"))
        };
    }
    if let Some(needle) = between(text, "the inspector shows the operation \"", "\"") {
        return if inspector_shows(w.h(), needle) {
            Ok(())
        } else {
            fail(format!("the inspector does not show operation {needle:?}"))
        };
    }
    if let Some(list) = text.strip_prefix("the selected shapes are ") {
        let want = sorted(nums(list));
        let got = sorted(w.state().selected_shapes.clone());
        return if got == want {
            Ok(())
        } else {
            fail(format!("expected selected shapes {want:?}, got {got:?}"))
        };
    }
    if let Some(label) = between(text, "the menu item \"", "\" is on screen") {
        let found = w
            .h()
            .query_all_by_label(label)
            .any(|n| n.accesskit_node().role() == egui::accesskit::Role::MenuItem);
        return if found {
            Ok(())
        } else {
            fail(format!("the menu item {label:?} is not on screen"))
        };
    }
    if let Some(label) = text.strip_suffix(" is on screen") {
        let label = unquote(label);
        return if has(w.h(), label) {
            Ok(())
        } else {
            fail(format!("{label:?} is not on screen"))
        };
    }
    if let Some(label) = text.strip_suffix(" is not on screen") {
        let label = unquote(label);
        return if has(w.h(), label) {
            fail(format!("{label:?} is on screen"))
        } else {
            Ok(())
        };
    }

    fail(format!("undefined step: {text:?}"))
}

fn run_scenario(s: &Scenario) -> Result<(), String> {
    let mut w = World::new();
    for (i, step_text) in s.steps.iter().enumerate() {
        step(&mut w, step_text).map_err(|e| format!("step {} ({step_text:?}): {e}", i + 1))?;
    }
    Ok(())
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    let full = if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "scenario panicked".to_string()
    };
    let head: Vec<&str> = full
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(4)
        .collect();
    let mut short = head.join(" ");
    if short.chars().count() > 300 {
        short = short.chars().take(300).collect::<String>() + "…";
    }
    short
}

fn write_gaps(
    dir: &Path,
    open: &[String],
    closed: &[String],
    failures: &[String],
    passed: usize,
) -> std::io::Result<()> {
    let mut out = String::new();
    out.push_str("# UI gaps\n\n");
    out.push_str(
        "Generated by `SPEC_INCLUDE_GAPS=1 cargo test -p stencilcraft --bin stencilcraft specs`.\n",
    );
    out.push_str(
        "Each open gap is a `@gap` scenario whose steps name the UI that does not exist yet.\n\n",
    );
    out.push_str(&format!(
        "Baseline scenarios passed: {passed}. Open gaps: {}. Closed gaps: {}. Baseline failures: {}.\n\n",
        open.len(),
        closed.len(),
        failures.len()
    ));
    out.push_str("## Open gaps\n\n");
    if open.is_empty() {
        out.push_str("None.\n\n");
    } else {
        for line in open {
            out.push_str(&format!("- {line}\n"));
        }
        out.push('\n');
    }
    out.push_str("## Closed gaps\n\n");
    if closed.is_empty() {
        out.push_str("None.\n\n");
    } else {
        for line in closed {
            out.push_str(&format!("- {line}\n"));
        }
        out.push('\n');
    }
    out.push_str("## Baseline failures\n\n");
    if failures.is_empty() {
        out.push_str("None.\n");
    } else {
        for line in failures {
            out.push_str(&format!("- {line}\n"));
        }
    }
    std::fs::write(dir.join("GAPS.md"), out)
}

#[test]
fn gherkin_specs() {
    let include_gaps = std::env::var("SPEC_INCLUDE_GAPS").is_ok_and(|v| v == "1");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("specs/features");
    let features = parse_dir(&dir);
    assert!(
        !features.is_empty(),
        "no .feature files under {}",
        dir.display()
    );

    let mut failures: Vec<String> = Vec::new();
    let mut open: Vec<String> = Vec::new();
    let mut closed: Vec<String> = Vec::new();
    let mut passed = 0usize;
    let mut skipped = 0usize;

    for feature in &features {
        for scenario in &feature.scenarios {
            let is_gap = scenario.tags.iter().any(|t| t == "gap");
            if is_gap && !include_gaps {
                skipped += 1;
                continue;
            }
            let outcome = catch_unwind(AssertUnwindSafe(|| run_scenario(scenario)));
            let (ok, message) = match outcome {
                Ok(Ok(())) => (true, String::new()),
                Ok(Err(msg)) => (false, msg),
                Err(payload) => (false, panic_message(payload)),
            };
            let file = feature
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| feature.path.display().to_string());
            let where_ = format!(
                "{}:{} — {} :: {}",
                file, scenario.line, feature.name, scenario.name
            );
            if ok {
                passed += 1;
                if is_gap {
                    closed.push(where_);
                }
            } else if is_gap {
                open.push(format!("{where_}\n  {message}"));
            } else {
                failures.push(format!("{where_}\n  {message}"));
            }
        }
    }

    if include_gaps {
        let specs_dir = dir.parent().unwrap_or(&dir);
        if let Err(e) = write_gaps(specs_dir, &open, &closed, &failures, passed) {
            eprintln!("could not write specs/GAPS.md: {e}");
        }
    }

    println!(
        "gherkin: {passed} passed, {} failed, {} gaps open, {} skipped",
        failures.len(),
        open.len(),
        skipped
    );

    if !failures.is_empty() {
        panic!(
            "{} baseline scenario(s) failed:\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
}
