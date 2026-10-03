# VDD configuration — Trofeo Vision 11.3"

`vdd_settings.xml` in this folder is an updated copy of the stock file at:

```
C:\Users\the\Downloads\VDD.Control.25.7.23\Dependencies\vdd_settings.xml
```

## What changed

| Setting | Stock | Updated |
|---|---|---|
| `<monitors><count>` | `1` | `2` |
| `1920x400` resolution | absent | present — landscape (`g_refresh_rate` applies) |
| `400x1920` resolution | absent | present — portrait |

Everything else (other resolutions, `gpu`, `options`) is unchanged.

## Why `count = 2`

VDD's `<resolutions>` list is **shared by all virtual monitors** — there is no per-monitor
resolution or EDID block in this driver version. Adding a second monitor and placing
`1920x400` (landscape) and `400x1920` (portrait) in the shared list lets that monitor be set
to either mounting. The original monitor can keep using its old resolution; you just leave
it set as it already is.

## Deploy

1. Close the VDD Control app (and any program using the virtual display).
2. Copy this file over the active settings file. The active path is one of:
   - the driver install directory (where `MttVDD.dll` lives), or
   - the VDD Control working directory (`...\VDD.Control.25.7.23\Dependencies\`).
   Safest: back up the current file first, then use **VDD Control** itself to apply the
   changes (it re-reads `vdd_settings.xml`), or replace the file and restart the VDD device.
3. In **Windows Settings -> System -> Display**, select the new monitor and set it to
   **1920x400** (horizontal strip) or **400x1920** (vertical strip). Position it
   (recommended: above/below or to a side, matching where the physical panel sits).
4. Verify with the bridge app:

   ```powershell
   cargo run --release -- --list-displays
   ```

   You should see a monitor reported at `1920x400` or `400x1920`; `trofeo-lcd-winmon`
   auto-detects which and rotates accordingly.

## `CustomEdid` is off on purpose

`<CustomEdid>true</CustomEdid>` uses a single `user_edid.bin` for **every** VDD monitor.
That would change the existing virtual monitor too. If you later want the Trofeo monitor to
report a specific EDID/name, you must accept that all VDD monitors share it (or use a VDD
version/build with per-monitor EDID).

## Revert

Restore your backed-up `vdd_settings.xml` and restart the VDD device. No driver files are
modified by this repo.
