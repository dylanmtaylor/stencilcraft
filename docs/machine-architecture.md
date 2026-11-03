# Machine driver architecture

StencilCraft drives cutters from any manufacturer through one vendor-neutral
pipeline in `crates/machine`. A job is described once; a driver made of three
independent parts turns it into bytes on a wire:

```
CutJob ──validate(Capabilities)──▶ Protocol::encode ──▶ CutProgram ──▶ Protocol::send ──▶ Transport
   ▲                                                       │ (dry run stops here)
 CLI / MCP / desktop                                  transcript / --out file
```

`MachineDriver = Capabilities + Protocol + Transport`. Nothing reaches a
transport without passing `validate`; encoding never touches hardware.

## Parts

### `CutJob` (`job.rs`)

Ordered `Operation`s, each with:

- `paths` — polylines in mm, `(x right, y down)`, origin at the media's top-left;
- `kind` — `Cut`, `Draw`, `Score`, `Perforate`, `Emboss`, `Engrave`, `Foil`;
- `tool` (`ToolKind`, or `None` = whatever is loaded) and `holder` (1-based);
- `settings` — `force`, `speed` (in the machine's own units), `passes`, `depth`
  (AutoBlade), `overcut_mm`, `blade_offset_mm`, corner sharpening.

Job-level fields: origin offset, end feed, orientation (`landscape`), loaded
`media_size_mm`, `mat`, `registration` (print & cut), `track_enhancing`,
`media_id`, `bbox_only` (placement preview). `CutSettings::to_job` builds the
single-operation job the desktop app and the first CLI used.

### `Capabilities` (`capabilities.rs`, data in `crates/machine/machines.toml`)

One row per machine: `protocol`, `support` (`supported` / `experimental` /
`export-only` / `unsupported`), USB `vvvv:pppp`, `firmware` reply needle, cut
width/length, media width, `roll_feed` (mat-free), `mats`, `tool_holders`,
`tools`, `force`/`speed` ranges **with units** (`steps`, `mm/min`, laser `S`),
`max_force_gf`, `bluetooth`, `regmarks`, `quad_regmarks`, `overcut`,
`multi_pass`, `notes`. An absent value means *not publicly documented*; the
validator treats it as "refuse" (force/speed → "set it on the panel"; cut width
→ "pass the media size"), never as "guess".

**Adding a model is data, not code**: add a `[[machine]]` row with an existing
`protocol`. Users can do the same without rebuilding: `--machines my.toml`
(CLI) loads a file of the same shape on top of the built-in table; rows with an
existing id replace it (e.g. tighter limits for a worn machine). Tables are
checked on load (unique ids, hex USB ids, ordered ranges, positive dimensions)
and a bad table is an error, never a panic.

Tool ids: `autoblade`, `ratchet`, `deep-cut`, `kraft`, `rotary`, `punch`, `pen`,
`emboss`, `deboss`, `etch`, `drag-blade`, `knife-blade`, `foil`, `score`,
`perforate`, `engrave`, `laser`.

### `Protocol` (`protocol/`)

```rust
trait Protocol {
    fn id(&self) -> ProtocolId;
    fn encode(&self, caps: &Capabilities, job: &CutJob) -> CutProgram;
    fn status(&self, t: &mut dyn Transport) -> Result<MachineStatus, _>; // default: Unsupported
    fn pause / resume / cancel(&self, t) -> Result<(), _>;              // default: Unsupported
    fn send(&self, t, caps, program, hooks) -> Result<SendReport, _>;   // default: stream
}
```

| `protocol` id | Implementation | Machines | Session |
|---|---|---|---|
| `silhouette-legacy`, `silhouette-cameo3`, `silhouette-cameo4` | `SilhouetteProtocol` (GP-GL) | Silhouette, Craft Robo | init, firmware check (a different known model aborts), ready poll, registration-mark wait; `ESC ENQ` status |
| `gpgl` | `GpglProtocol` | Graphtec CE, generic GP-GL | stream |
| `hpgl` | `HpglProtocol` (HP-GL / HP-GL/2 `IN SP PU PD` subset) | Roland, USCutter, Vinyl Express, many Chinese plotters | stream |
| `dmpl` | `DmplProtocol` (experimental) | DMPL cutters | stream |
| `gcode` | `GcodeProtocol` (Grbl) | lasers, knife/pen CNCs | send-response with `ok` acks, error → `M5` + abort; `?` status, `!`/`~` pause/resume, `0x18` cancel |
| `export-only`, `unsupported` | `UnsupportedProtocol` | Curio, Cricut | never encodes or sends |

Placeholders for further languages (Roland CAMM-GL III extensions, Summa DM/PL,
Mimaki, Brother ScanNCut) are added the same way: a new `ProtocolId`, an
implementation, and rows in the table.

### `Transport` (`transport/`)

```rust
trait Transport {
    fn write(&mut self, data: &[u8]) -> Result<usize, _>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, _>;          // Ok(0) = nothing arrived
    fn read_timeout(&mut self, buf, timeout) -> Result<usize, _>;
    fn interactive(&self) -> bool;  // false for spools: sessions skip the handshake
    fn is_open(&self) -> bool;
    fn close(&mut self) -> Result<(), _>;
    fn describe(&self) -> String;
}
```

Opening is by constructor or by `TransportSpec::parse(..).open()`; opening
never sends a byte.

| Spec | Type | Feature | Notes |
|---|---|---|---|
| `usb`, `usb:BUS-ADDR` | `UsbTransport` | `usb` | `nusb` 0.2 (MIT OR Apache-2.0): bulk OUT/IN on interface 0, kernel driver detached, 5 s timeouts. Not yet exercised on hardware. |
| `/dev/ttyUSB0[@BAUD]`, `serial:COM3` | `SerialTransport` | `serial` | USB-CDC (Grbl/Marlin boards, many plotters), RS-232, OS-paired Bluetooth RFCOMM ports |
| `tcp:HOST:PORT` | `TcpTransport` | — | raw sockets (port 9100 print servers, serial-to-Ethernet bridges, Grbl over Wi-Fi) |
| `file:PATH`, `stdout` | `FileTransport` | — | spool; not interactive |
| — | `MockTransport`, `RecordingTransport` | — | scripted replies / dry-run capture for tests |

**Bluetooth (future).** No permissive cross-platform Rust crate makes RFCOMM
straightforward today; BLE GATT (`btleplug`, MIT/Apache) would also need the
GATT service layout, which is not publicly documented for these cutters. The
slot is a `BluetoothTransport` implementing the same trait; until then, pair in
the OS and use the serial transport on the RFCOMM port.

### `DeviceRegistry` (`registry.rs`)

The built-in table (`DeviceRegistry::builtin()`), user overrides
(`with_overrides`), and discovery:

- `by_id`, `by_usb(vid, pid)`, `by_firmware("CAMEO 4 PRO V1.02")` (longest needle wins);
- `match_usb(enumerated)` — with `list_usb_devices()` (feature `usb`) for USB discovery;
- `probe(transport)` — sends the read-only GP-GL `ESC EOT` + `FG` and identifies
  the answer, over serial, TCP or USB;
- manual selection by id (`--model ID`).

### Status and events

`MachineStatus`: `Ready`, `Busy`, `Paused`, `MediaUnloaded`, `Error`, `Unknown`.
A send reports `DriverEvent`s (`Identified`, `Status`, `Waiting`, `Sent`,
`Done`) through `SessionHooks`, which also supplies the pause between polls.
Pause/cancel exist where the protocol defines them (Grbl); elsewhere they
return `MachineError::Unsupported` rather than pretending.

## Safety

- Dry run is the default everywhere. MCP has no send method at all.
- `MachineDriver::send` validates against the capabilities before any byte goes
  out; the CLI's `--device` is the only deliberate real-send step.
- The Silhouette session refuses to cut if the firmware names another known
  model, if media is unloaded, if the device never reports ready, or if
  registration marks are not found.
- Undocumented limits are refused, not guessed; caps on points (2 000 000),
  operations (64), passes (10) and overcut (2 mm) bound untrusted documents.

## Cricut

Registry rows exist for Joy, Joy Xtra, Explore Air 2, Explore 3, Maker, Maker 3
and Venture with capabilities from public product pages (unverified), all with
`protocol = "unsupported"`. They are listed and inspectable; validation rejects
every job with the export workaround. Implementing the protocol is not
approved: it is proprietary, reportedly encrypted and tied to the vendor's
cloud app, and needs an owner decision and legal review first.

## Surfaces

- CLI: `stencilcraft-cli machines [--json] [--machines FILE] [show ID]`,
  `devices [--status] [--probe DEV]`, `cut --model ID [--machines FILE] [--out FILE] [--device DEV] …`.
- MCP: `machines_list`, `machine_capabilities`, `machine_dry_run` (`docs/mcp.md`).
- Desktop: the existing `Model` enum + `encode_cut` preview keep working; UI
  wiring for the registry is left to the UI owner (command names above).
