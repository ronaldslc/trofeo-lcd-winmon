# Research — Trofeo Vision 11.3" as a real Windows second monitor

Everything below was gathered from public sources. Each claim is followed by its
reference. This document is intentionally verbose so the implementation can be
audited against primary sources.

## 1. Device identity

| Property | Value | Source |
|---|---|---|
| Vendor ID | `0x0416` | InfoPanel fork model DB; `trofeo-lcd-rs` |
| Product ID (LY) | `0x5408` | same |
| Product ID (LY1) | `0x5409` | same |
| USB product string | `USBDISPLAY` | user's `usbipd list` |
| Transport | WinUSB bulk | InfoPanel release notes; `trofeo-lcd-rs` |
| Protocol family | "LY" / `TrofeoBulk` | `trofeo-lcd-rs`; InfoPanel |
| 11.3" native resolution | `1920x400` | InfoPanel fork model DB |
| 9.16" native resolution | `1920x462` | `trofeo-lcd-rs` |

The 11.3" shares **PID `0x5408`** with the 9.16". InfoPanel disambiguates them at runtime
by a handshake response byte (`byte[20] == 0x05`). The 11.3" firmware still *reports*
`1920x480` in the init response, but the physical panel is `1920x400`, and InfoPanel
renders at `1920x400`:

- https://github.com/emaspa/infopanel-1/blob/all-changes/InfoPanel/ThermalrightPanel/ThermalrightPanelModelDatabase.cs
  (see the comment block around `TrofeoVision113`).

## 2. The protocol ("LY" / TrofeoBulk)

Reverse-engineered in the open-source projects below. Summary (full spec in
`docs/PROTOCOL.md`):

- Init handshake: write `16 + 2032` bytes (header `02 FF ... 01 ...`), read a 512-byte
  response that carries `PM`/`SUB` bytes.
- Frames are **JPEG** images, not raw pixels.
- JPEG bytes are chopped into `496`-byte payloads inside `512`-byte chunks (16-byte
  header), then written in **4096-byte bulk writes**, followed by a 512-byte ACK.
- Firmware cap: **~450,000 bytes** per JPEG frame.

References:

- https://github.com/haven80/trofeo-lcd-rs — `src/lib.rs` (constants, `build_chunks`,
  `send_frame`); `src/bin/screen.rs`; `src/dxgi_capture.rs`
- https://github.com/sukualam/trofeo-lcd — original Rust project
- https://github.com/Lexonight1/thermalright-trcc-linux — Python reference driver
- https://github.com/emaspa/infopanel-1/blob/all-changes/PANELS.md — panel/protocol notes
- https://github.com/habibrehmansg/infopanel — upstream InfoPanel

## 3. Precedent: "second monitor mode" already exists for the 9.16"

`trofeo-lcd-rs` ships `GUIDE_SECOND_MONITOR.md`, which describes exactly the target
architecture for the 9.16" (same USB stack):

1. Install an **IddCx Virtual Display Driver (VDD)** and add a `1920x462` mode.
2. `trofeo_screen.exe` captures that virtual monitor with the **DXGI Desktop Duplication**
   API and streams JPEG to the panel over USB bulk.

- https://github.com/haven80/trofeo-lcd-rs/blob/main/GUIDE_SECOND_MONITOR.md
- https://github.com/haven80/trofeo-lcd-rs/blob/main/src/dxgi_capture.rs

Our project is a clean-room reimplementation of only this functionality, parameterised for
`1920x400` and using **Windows Graphics Capture (WGC)** instead of DXGI duplication
(WGC includes the cursor reliably and is simpler). This is not a fork and contains no code
from those projects.

## 4. Why a "USB monitor driver" does not exist off the shelf

- Windows has **no inbox driver** for arbitrary USB displays. The mainstream options are:
  - **DisplayLink** — proprietary USB graphics chipsets; not this device.
  - **USB-C DisplayPort alt-mode** — not USB data at all; not this device.
- The known way to expose a non-standard USB panel as a monitor is to synthesise a virtual
  monitor (IddCx) and forward the pixels, or use a WDDM filter driver like Fresco Logic's
  `lci_proxykmd` (see below).
- `LinJiabang/virtual-display` is a sample "USB/Ethernet Display driver for Windows" built on
  Fresco Logic's LCI Proxy WDDM filter. It proves the concept but is licensed/chipset-bound
  and not applicable here: https://github.com/LinJiabang/virtual-display
- Microsoft's IddCx overview:
  https://learn.microsoft.com/en-us/windows-hardware/drivers/display/indirect-display-driver-model-overview

**Conclusion:** the VDD (IddCx) + capture/bridge split is the correct, lowest-risk path.

## 5. Virtual Display Driver (VDD)

- Canonical repo (formerly `itsmikethetech/Virtual-Display-Driver`):
  https://github.com/VirtualDrivers/Virtual-Display-Driver
- Supports **custom resolutions and custom EDID**.
- Config lives in `vdd_settings.xml` next to the driver, managed by "VDD Control".
- Installed here: `C:\Users\the\Downloads\VDD.Control.25.7.23`
  - Driver: `SignedDrivers\x86\VDD\MttVDD.dll` (+ `MttVDD.inf`), IddCx `0102`.
  - Settings: `Dependencies\vdd_settings.xml` (schema read from the installed file).

### Schema limits found in the installed version

The installed `vdd_settings.xml` is **global**, not per-monitor:

```xml
<monitors><count>1</count></monitors>
<g_resolution>... a shared <resolutions> list ...</g_resolution>
<options>
  <CustomEdid>false</CustomEdid>   <!-- ONE user_edid.bin for ALL virtual monitors -->
  ...
</options>
```

Implication: adding a Trofeo monitor means bumping `<count>` to `2` and adding the
`1920x400` resolution to the shared list. A custom EDID (if enabled) would apply to **all**
VDD monitors, so we deliberately leave `CustomEdid=false`. See `vdd/README.md`.

## 6. Capture API chosen

- **Windows Graphics Capture (WGC)** via the `windows-capture` crate, v2.x:
  https://github.com/NiiightmareXD/windows-capture
- Alternative (used by the 9.16" guide): DXGI Desktop Duplication
  https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api
- Rationale for WGC: per-monitor targeting, cursor capture built in, only emits frames on
  change (naturally idles), and far less unsafe plumbing than raw Desktop Duplication.

## 7. Supporting crates

| Crate | Purpose | Link |
|---|---|---|
| `windows-capture` | WGC monitor capture with cursor | https://crates.io/crates/windows-capture |
| `rusb` | WinUSB access via libusb | https://crates.io/crates/rusb |
| `jpeg-encoder` | Pure-Rust JPEG encode | https://crates.io/crates/jpeg-encoder |
| `anyhow` / `thiserror` | error handling | https://crates.io/crates/anyhow |

## 8. Key takeaways that shape the implementation

1. The 11.3" is `1920x400` and uses the same `0416:5408` LY protocol as the 9.16".
2. Only the **resolution** and the 11.3"-vs-9.16" **handshake byte** differ.
3. The proven architecture is **virtual monitor + capture + USB JPEG bridge**.
4. The virtual monitor must exist first (VDD); the bridge app does the rest.
5. Only one process may own the panel at a time (TRCC/InfoPanel must be closed).
6. USB bulk JPEG realistically yields ~10-30 fps: excellent for widgets/secondary content,
   not for gaming.
7. **Keep-alive is mandatory.** The panel firmware reverts to its built-in Thermalright logo
   when it stops receiving frames (observed on real hardware; analogous to the documented
   "reverts to its built-in animation if keep-alive is missed for >1.5 s" behavior for the
   Hongtai/JL family in InfoPanel's `PANELS.md`). Because WGC only emits frames on change, the
   bridge must periodically re-send the last frame. Our sender thread does this every
   `keepalive_ms` (default 400 ms).
