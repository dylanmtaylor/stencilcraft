//! StencilCraft's host of the framework runtime: the embed hooks its eframe `App` calls
//! (docs/craft-framework.md section 2.2 of craft-libs), and the reads of the keys the runtime
//! routes to the window instead of to egui.

use std::sync::{Arc, OnceLock};

use eframe::egui;

use craft_host::target::{Backend, ShellInfo};
use craft_host::{AppInfo, Host};
use craft_keymap::{Chord, Key, Mods};
use craft_ui::fonts::{I18nScripts, UiFonts};
use craft_ui::runtime::InstallSpec;
use craft_ui::runtime::env::SystemReader;
use craft_ui::runtime::focus::{EditSet, KeyUse};
use craft_ui::runtime::frame::FrameInput;
use craft_ui::runtime::keys::{Intent, with_window_intents};
use craft_ui::shell::embed::{self, CloseDecision};
use craft_window::system::{NoWindows, SystemWindows, WindowSystem};

/// The settings folder name and display name the framework's dialogs and the window host use.
const APP: AppInfo = AppInfo {
    id: "stencilcraft",
    name: "StencilCraft",
    version: env!("CARGO_PKG_VERSION"),
};

/// The app's services, built once so every frame hands the runtime the same host.
fn host() -> Arc<Host> {
    static HOST: OnceLock<Arc<Host>> = OnceLock::new();
    HOST.get_or_init(|| Arc::new(Host::builder(APP).build()))
        .clone()
}

/// Installs the runtime on the native app's context, with the platform's windows and the
/// window frame `frame` decided at start, then the interface fonts. Call once, from eframe's
/// creator.
pub fn install(ctx: &egui::Context, frame: craft_window::policy::Decision) -> Result<(), String> {
    let repaint = ctx.clone();
    let system = SystemReader::start(move || repaint.request_repaint());
    let spec = spec(
        ShellInfo::default().backend(Backend::from_env()),
        Arc::new(SystemWindows),
    )
    .system(system)
    .frame(frame);
    install_spec(ctx, spec)
}

/// Installs the runtime with no window system: for every test harness that runs the app's UI.
pub fn install_headless(ctx: &egui::Context) -> Result<(), String> {
    install_spec(ctx, spec(ShellInfo::default(), Arc::new(NoWindows)))
}

fn spec(shell: ShellInfo, windows: Arc<dyn WindowSystem>) -> InstallSpec {
    InstallSpec::new(shell, windows).app_id(APP.id)
}

fn install_spec(ctx: &egui::Context, spec: InstallSpec) -> Result<(), String> {
    let fonts = UiFonts::new(&I18nScripts);
    embed::install(ctx, spec, &fonts)
        .map(|_| ())
        .map_err(|error| format!("the runtime could not be installed: {error}"))
}

/// Before each frame, from `raw_input_hook`: the app's theme as the runtime's root, and its host.
/// StencilCraft binds neither F6 nor Tab, so no framework binding is unbound.
pub fn before_input(ctx: &egui::Context, theme: stencilcraft_ui_egui::ColorTheme) {
    embed::before_input(ctx, |_output| {
        FrameInput::new(stencilcraft_ui_egui::root_env(theme)).host(host())
    });
}

/// Each pass, first in `ui`: hands the runtime the native window.
pub fn before_ui(ctx: &egui::Context, frame: &eframe::Frame) {
    embed::before_ui(ctx, frame);
}

/// Each frame, last in `logic`: the root window's close path. StencilCraft autosaves and keeps
/// recovered designs, so a close request closes the window, as it did before the runtime.
pub fn close_path(ctx: &egui::Context) {
    embed::logic(ctx, || CloseDecision::Close);
}

/// ⌘/Ctrl+Alt with a key.
const fn cmd_alt(key: Key) -> Chord {
    Chord::new(
        Mods {
            cmd: true,
            ctrl: false,
            alt: true,
            shift: false,
        },
        key,
    )
}

/// The app's ⌘/Ctrl+Alt+digit shortcuts (Unlock All, Show All, Zoom to Selection, Release
/// Guides). Claimed, so Alt+numpad code entry on Windows and Linux leaves them to the app.
const CANVAS_CLAIMS: &[Chord] = &[
    cmd_alt(Key::Num0),
    cmd_alt(Key::Num2),
    cmd_alt(Key::Num3),
    cmd_alt(Key::Num5),
];

/// Declares the app's own canvas widget `id` the window's document canvas for this pass. The
/// edit chords, egui's clipboard events and the claimed chords stay in egui's input for it,
/// where the app's own shortcut table reads them.
pub fn host_canvas(ctx: &egui::Context, id: egui::Id) {
    craft_ui::runtime::focus::host_canvas(
        ctx,
        id,
        KeyUse::CANVAS
            .with_edits(EditSet::ALL)
            .with_claims(CANVAS_CLAIMS),
    );
}

/// Takes this pass's window Escape (`Intent::Cancel`); whether there was one.
pub fn take_window_cancel(ctx: &egui::Context) -> bool {
    with_window_intents(ctx, false, |intents| intents.take(Intent::Cancel))
}

/// Gives `ui` the app's own egui style, for panel bodies and the canvas group view inside the
/// dock, which would otherwise lay the app's code out in the dock's spacing.
pub fn app_style(ui: &mut egui::Ui) {
    ui.set_style(ui.ctx().global_style());
}
