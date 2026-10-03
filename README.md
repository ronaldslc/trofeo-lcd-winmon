# trofeo-lcd-winmon

Clean-room **"Second Monitor Mode"** bridge for the **Thermalright Trofeo Vision 11.3"**
USB LCD (`VID:PID = 0416:5408`, reported by `usbipd` as `USBDISPLAY`).

The goal is to make Windows list the panel as a **real secondary monitor** (drag windows
onto it, extend the desktop), instead of being limited to TRCC/InfoPanel widgets.

## How it works

Windows has no inbox/standard driver for this panel (it is a proprietary USB bulk JPEG
sink, not DisplayLink, not USB-C DP-alt-mode). So the monitor is synthesised and the pixels
are forwarded:

```
  +---------------------+     +----------------------+     +-----------------------+
  | Virtual Display      |     | trofeo-lcd-winmon        |     | Trofeo Vision 11.3"   |
  | Driver (IddCx)       | --> | Windows Graphics     | --> | WinUSB bulk           |
  | virtual 1920x400     |     | Capture (WGC) ->     |     | "LY" protocol         |
  | monitor "Display 2"  |     | scale -> JPEG        |     | 0416:5408             |
  +---------------------+     +----------------------+     +-----------------------+
```

1. **VDD / IddCx** creates a virtual monitor at the panel's native `1920x400`.
2. **trofeo-lcd-winmon** captures that monitor (Windows Graphics Capture, cursor included),
   converts BGRA -> RGB, scales to `1920x400`, JPEG-encodes it, and streams it to the
   panel over USB bulk using the reverse-engineered "LY" protocol.
3. Windows genuinely sees "Display 2"; this process is what lights the physical panel.

This is the same architecture proven on the 9.16" sibling (see `docs/RESEARCH.md`), but
**written from scratch** and parameterised for the 11.3" (`1920x400`).

## Repository layout

```
docs/RESEARCH.md   Findings + references (what is known, and where it came from)
docs/PLAN.md       Implementation plan, phases, risks
docs/PROTOCOL.md   Clean-room wire specification of the "LY" USB bulk protocol
vdd/               Updated VDD settings that add the Trofeo 11.3 virtual monitor
src/               Rust source (clean-room implementation)
```

## Requirements

- Windows 10 1903+ / Windows 11 (Windows Graphics Capture).
- **Rust** (MSVC toolchain) — installed `1.99.0`, target `stable-x86_64-pc-windows-msvc`.
- **Virtual Display Driver (VDD)** already installed (yes) — see `vdd/README.md`.
- The panel must be bound to **WinUSB** (as TRCC/InfoPanel already use it).

## Build

```powershell
cargo build --release
```

This project is already verified: `cargo build`, `cargo build --release`, `cargo clippy
--all-targets` (clean), and `cargo test` (4/4 passing) all succeed with Rust 1.99.0.

## Run

```powershell
# show detected monitors (find the 1920x400 virtual one)
cargo run --release -- --list-displays

# stream the virtual monitor to the panel
cargo run --release -- --display 1

# sharper text
cargo run --release -- --quality 85 --fps 25

# keep-alive cadence (resend last frame when desktop is idle)
cargo run --release -- --keepalive-ms 400
```

> If the app is currently running, stop it before rebuilding — Windows keeps
> `trofeo-lcd-winmon.exe` locked while it is in use.

## Autostart at logon

The app must run in your **interactive user session** (it captures the desktop), so a
Windows *service* will not work. Use the Startup folder (simplest, no admin) or a logon
Scheduled Task (delay + auto-restart).

### Option A — Startup folder (recommended)

```powershell
cargo build --release
powershell -ExecutionPolicy Bypass -File .\scripts\install-autostart.ps1
```

This copies the binary to `%LOCALAPPDATA%\TrofeoWinMon\trofeo-lcd-winmon.exe` and adds a
**Trofeo LCD WinMon** shortcut to `shell:startup`, launching with `--hide-console`. It survives
`cargo clean` because it runs from the copied location.

Pass extra options with `-ExtraArgs`:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-autostart.ps1 `
  -ExtraArgs "--hide-console --quality 85 --fps 20"
```

Remove it with:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\uninstall-autostart.ps1 -StopRunning
```

Restart after a rebuild (stops the old instance, copies the new binary, starts it):

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-autostart.ps1 -Restart
```

Or restart the already-installed copy without rebuilding:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\restart-autostart.ps1
```

### Option B — Scheduled Task (delay / restart)

Use this if the VDD monitor is not always ready at logon, or you want automatic restart on
failure (may require an elevated PowerShell):

```powershell
$exe = "$env:LOCALAPPDATA\TrofeoWinMon\trofeo-lcd-winmon.exe"
$action  = New-ScheduledTaskAction -Execute $exe -Argument "--hide-console"
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
$trigger.Delay = "PT15S"
$settings = New-ScheduledTaskSettingsSet -RestartCount 3 `
  -RestartInterval (New-TimeSpan -Minutes 1) -ExecutionTimeLimit 0
Register-ScheduledTask -TaskName "Trofeo LCD WinMon" -Action $action -Trigger $trigger `
  -Settings $settings -Force
```

Remove it with `Unregister-ScheduledTask -TaskName "Trofeo LCD WinMon" -Confirm:$false`.

### Notes

- Ensure **TRCC / InfoPanel are closed**; only one process may own the USB device.
- Run only **one instance** of `trofeo-lcd-winmon`.
- `--hide-console` hides the window but keeps `stdout`/`stderr`; run it once in a terminal
  (without the flag) to confirm it works before relying on autostart.

Make sure **TRCC and InfoPanel are closed**: only one program may own the USB device.

See `docs/PLAN.md` for the full option list and troubleshooting.

## VDD display setup

`trofeo-lcd-winmon --list-displays` (build verified) reports:

```
[ 0] DISPLAY1                       \\.\DISPLAY1     5120x1440 @240Hz
[ 1] DISPLAY2                       \\.\DISPLAY5     3840x2160 @60Hz
```

After applying `vdd/vdd_settings.xml` there will be an additional VDD monitor to set to **1920x400**, which `trofeo-lcd-winmon` will auto-detect.

## Idle / keep-alive (important)

The panel firmware reverts to its built-in Thermalright logo if it stops receiving frames.
Windows Graphics Capture only emits frames when the desktop *changes*, so an idle screen
would otherwise send nothing. A dedicated sender thread therefore re-sends the last frame
every `--keepalive-ms` (default **400 ms**) when nothing new has arrived. That keeps the
panel showing the desktop indefinitely. Lower the value if the logo still appears; raise it
to reduce idle USB traffic.

## Orientation / portrait mode

The panel is physically a **1920x400** strip. If you mount it **vertically**, create the
virtual monitor at **400x1920** instead — `trofeo-lcd-winmon` detects the resolution from the
monitor and rotates the captured frame back to the panel's native `1920x400` automatically.

`--list-displays` marks the match:

| Marker | Meaning |
|---|---|
| `MATCH (landscape)` | monitor is `1920x400`, sent as-is |
| `MATCH (portrait)` | monitor is `400x1920`, rotated 90° |

Overrides:

| Flag | Meaning |
|---|---|
| `--orientation auto\|landscape\|portrait` | force which mode to look for (default `auto`) |
| `--portrait-ccw` | portrait rotation direction: counter-clockwise instead of the default clockwise |
| `--rotate-180` | extra 180° (upside-down mounting); combines with the auto rotation |

Examples:

```powershell
# auto-detect landscape or portrait
.\target\release\trofeo-lcd-winmon.exe

# force portrait, and use the other rotation direction if the image is upside-down
.\target\release\trofeo-lcd-winmon.exe --orientation portrait --portrait-ccw
```

`vdd/vdd_settings.xml` already includes both `1920x400` and `400x1920`, so the VDD monitor
can be set to whichever matches how the strip is mounted.

## License

GNU GPL v3.0-or-later — see [`LICENSE`](./LICENSE).

The protocol implemented here derives from GPL-licensed reverse-engineering projects; see
[`NOTICE`](./NOTICE) for provenance/credits and [`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md)
for dependency licenses (including the LGPL libusb notice).
