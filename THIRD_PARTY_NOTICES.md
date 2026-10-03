# Third-party notices

This project depends on the following crates/binaries. Their licenses are repeated here for
convenience; the authoritative texts ship with each package.

| Dependency | License | Notes |
|---|---|---|
| [`windows-capture`](https://crates.io/crates/windows-capture) | MIT | Windows Graphics Capture bindings |
| [`rusb`](https://crates.io/crates/rusb) | MIT | USB access |
| [`libusb`](https://github.com/libusb/libusb) (vendored via `rusb`/`libusb1-sys`) | **LGPL-2.1-or-later** | statically linked by default (see below) |
| [`jpeg-encoder`](https://crates.io/crates/jpeg-encoder) | (MIT OR Apache-2.0) AND IJG | JPEG encoding; includes IJG-derived code |
| [`anyhow`](https://crates.io/crates/anyhow) | MIT OR Apache-2.0 | error handling |
| [`thiserror`](https://crates.io/crates/thiserror) | MIT OR Apache-2.0 | error derive |
| [`windows`](https://crates.io/crates/windows) | MIT OR Apache-2.0 | `--hide-console` |

## libusb (LGPL-2.1-or-later)

`rusb` is built with the `vendored` feature, which **statically links libusb**. libusb is
licensed under the GNU Lesser General Public License v2.1-or-later.

Because this project as a whole is distributed under GPL-3.0-or-later, the corresponding
source (including this project and its dependency sources obtainable from crates.io / the
linked upstreams) is made available, and recipients may rebuild/relink the program against
a modified libusb. If you redistribute a binary, keep this notice and provide the source
(or a written offer) as required by the LGPL/GPL.

If you prefer to avoid static LGPL linking entirely, build `rusb` against a shared
`libusb-1.0.dll` instead of the `vendored` feature.

## IJG (jpeg-encoder)

`jpeg-encoder` contains code derived from the Independent JPEG Group's reference
implementation; its license requires that the "IJG" origin be acknowledged. See the
`jpeg-encoder` package for the full text.
