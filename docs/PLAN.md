# Implementation plan

## Goal

Make the Thermalright Trofeo Vision 11.3" (`0416:5408`) appear to Windows as a **real
secondary monitor** and stream the desktop to it, with a clean-room implementation that
contains **only** second-monitor functionality (no widget/monitor/hud features).

## Architecture

Two components, cleanly separated:

```
[ VDD / IddCx ]  ->  creates  "Display N"  @ 1920x400   (existing driver, config only)
        |
        v   WGC captures that monitor (cursor included)
[ trofeo-lcd-winmon ]  ->  BGRA -> RGB -> scale 1920x400 -> JPEG -> LY bulk protocol
        |
        v
[ Trofeo Vision 11.3" ]  ->  displays the stream
```

Why split? An IddCx driver cannot portably own a WinUSB device from its software stack.
The two-component design is simpler, already proven on the 9.16" sibling, and lets us
change resolution/quality without touching a signed driver.

## Deliverables in this repo

- `vdd/vdd_settings.xml` — updated settings adding a second monitor + `1920x400` mode.
- `vdd/README.md` — how to deploy/revert.
- `src/*` — the clean-room Rust bridge (capture + sender thread with keep-alive).
- `scripts/install-autostart.ps1` / `scripts/uninstall-autostart.ps1` — Startup-folder autostart.

## Phases

### Phase 0 — Recon (done)
- Identify device, PID, protocol, resolution. See `docs/RESEARCH.md`.

### Phase 1 — Virtual monitor
- Apply `vdd/vdd_settings.xml` (via VDD Control), set `<count>2</count>`, add `1920x400`.
- In Windows Display Settings, set the new "Display 2" to **1920x400**, position it as
  desired, and (recommended) set it as a secondary/extended display.
- Confirm with `trofeo-lcd-winmon --list-displays` that a `1920x400` monitor is visible.

### Phase 2 — USB bring-up
- `TrofeoDevice::open()` finds `0416:5408` or `0416:5409`, claims the bulk interface,
  performs the handshake, and reports `PM`/`SUB` (the 11.3" is expected to report the
  11.3" marker; see `docs/PROTOCOL.md`).
- Smoke test: send a solid-colour/one-shot JPEG to confirm the panel reacts.

### Phase 3 — Capture bridge
- `capture.rs` finds a `1920x400` (landscape) or `400x1920` (portrait) monitor, starts WGC
  with `CursorCaptureSettings::WithCursor`, and derives the rotation from the monitor's
  dimensions (portrait -> 90°, direction configurable).
- Each frame: BGRA -> RGB -> rotate to native landscape -> scale to `1920x400` -> JPEG,
  published to a shared mailbox.
- `sender.rs` owns the USB device on its own thread and: sends new frames immediately, and
  **re-sends the last frame every `keepalive_ms` when idle**. This is required because the
  panel firmware falls back to its built-in logo when it stops receiving frames, and WGC
  only emits frames on change.
- `MinimumUpdateIntervalSettings::Custom(1/fps)` caps the upload rate.

### Phase 4 — Robustness
- Reconnect on USB errors (device reset / replug).
- Clear errors when the monitor disappears.
- Optional: auto-restart, hide console, tray icon.

## CLI

| Flag | Meaning | Default |
|---|---|---|
| `--list-displays` | List monitors and exit | — |
| `--display <N>` | Monitor index to capture | auto-detect `1920x400` / `400x1920` |
| `--orientation auto\|landscape\|portrait` | Which mounting to look for | `auto` |
| `--portrait-ccw` | Portrait rotation: counter-clockwise | off (clockwise) |
| `--width <N>` / `--height <N>` | Target panel resolution | `1920` / `400` |
| `--fps <N>` | Max upload frame rate | `25` |
| `--quality <1-100>` | Base JPEG quality | `75` |
| `--keepalive-ms <N>` | Resend last frame every N ms when idle | `400` |
| `--rotate-180` | Rotate 180° (upside-down mounting) | off |
| `--probe-only` | Handshake the device, don't stream | off |
| `--test-pattern` | Send one colour-bar frame (USB bring-up) | off |

## Risks / notes

- **One owner only** — stop TRCC/InfoPanel; they open the same USB device.
- **Driver binding** — the panel must be on WinUSB for `rusb`/libusb to open it. TRCC/InfoPanel
  already imply this; if `open()` fails, verify the device's driver in Device Manager.
- **Signed driver** — VDD is already signed; we never build a driver ourselves.
- **Refresh** — USB bulk JPEG caps out around 10-30 fps. Fine for dashboards/secondary apps,
  not for video/gaming.
- **Latency** — WGC + JPEG + USB adds tens of milliseconds.
- **Bandwidth** — `1920x400` at 75% JPEG is ~40-90 KB/frame; ~25 fps stays within USB 2.0
  bulk budget.
- **EDID** — the installed VDD treats `user_edid.bin` as global. We leave it off so the
  existing virtual monitor is unaffected. The Windows monitor will show as a generic
  "Virtual Display"; that is cosmetic.
- **11.3" vs 9.16" detection** — both share PID `0x5408`; we key off the handshake marker
  and simply render at `1920x400`.

## Verification status

The scaffold compiled **cleanly on the first attempt**:

- `cargo build` — ok
- `cargo build --release` — ok (`target/release/trofeo-lcd-winmon.exe`)
- `cargo clippy --all-targets` — no warnings
- `cargo test` — 4/4 protocol tests pass

Toolchain: Rust 1.99.0, `stable-x86_64-pc-windows-msvc`. Key versions resolved:
`windows-capture 2.0.1`, `windows 0.62.2`, `rusb 0.9.4` (libusb vendored), `jpeg-encoder 0.6.1`.

Additional features verified after the first build:

- Idle **keep-alive** (panel no longer falls back to the logo).
- `--hide-console` and the autostart scripts (compile-verified via `cargo check` / `clippy`; a
  fresh release link is pending because the currently running `trofeo-lcd-winmon.exe` holds the
  output file lock).

### Bring-up order once hardware is available

1. `trofeo-lcd-winmon --list-displays` — find the `1920x400` VDD monitor.
2. `trofeo-lcd-winmon --test-pattern` — confirm the USB path (colour bars on the panel).
3. `trofeo-lcd-winmon --display <N>` — full stream.

These steps were **not** executed here (they touch the physical device/driver). Only
read-only commands (`--help`, `--list-displays`) were run.
