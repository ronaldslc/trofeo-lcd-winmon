//! Command-line configuration.

use anyhow::{bail, Result};

pub const DEFAULT_WIDTH: u32 = 1920;
pub const DEFAULT_HEIGHT: u32 = 400;
pub const DEFAULT_FPS: f32 = 25.0;
pub const DEFAULT_QUALITY: u8 = 75;
pub const DEFAULT_KEEPALIVE_MS: u64 = 400;

/// How the panel is mounted / which virtual monitor to capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Detect from the monitor's resolution (1920x400 landscape or 400x1920 portrait).
    Auto,
    /// Landscape strip (monitor 1920x400).
    Landscape,
    /// Portrait / vertical strip (monitor 400x1920).
    Portrait,
}

impl Orientation {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Some(Orientation::Auto),
            "landscape" | "horizontal" | "h" => Some(Orientation::Landscape),
            "portrait" | "vertical" | "v" => Some(Orientation::Portrait),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub list_displays: bool,
    pub probe_only: bool,
    pub test_pattern: bool,
    pub hide_console: bool,
    pub display_index: Option<usize>,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub quality: u8,
    pub keepalive_ms: u64,
    pub rotate_180: bool,
    pub orientation: Orientation,
    pub portrait_ccw: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            list_displays: false,
            probe_only: false,
            test_pattern: false,
            hide_console: false,
            display_index: None,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            fps: DEFAULT_FPS,
            quality: DEFAULT_QUALITY,
            keepalive_ms: DEFAULT_KEEPALIVE_MS,
            rotate_180: false,
            orientation: Orientation::Auto,
            portrait_ccw: false,
        }
    }
}

pub const HELP: &str = "\
trofeo-lcd-winmon — stream a virtual monitor to the Trofeo Vision 11.3\" USB LCD

USAGE:
    trofeo-lcd-winmon [OPTIONS]

OPTIONS:
    --list-displays        List monitors and exit
    --display <N>          Monitor index to capture (default: auto-detect WIDTHxHEIGHT)
    --orientation <MODE>   auto | landscape | portrait (default: auto)
    --portrait-ccw         Portrait rotation direction: counter-clockwise (default: cw)
    --width <N>            Target width  (default: 1920)
    --height <N>           Target height (default: 400)
    --fps <N>              Max upload frame rate (default: 25)
    --quality <1-100>      Base JPEG quality (default: 75)
    --keepalive-ms <N>     Resend the last frame every N ms when idle (default: 400)
    --rotate-180           Rotate 180 degrees (upside-down mounting)
    --probe-only           Handshake the device and exit (no streaming)
    --test-pattern         Send one colour-bar frame and exit (USB bring-up)
    --hide-console         Hide the console window after start (for autostart)
    -h, --help             Show this help
";

impl Config {
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Self> {
        let mut cfg = Config::default();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--list-displays" => cfg.list_displays = true,
                "--probe-only" => cfg.probe_only = true,
                "--test-pattern" => cfg.test_pattern = true,
                "--hide-console" => cfg.hide_console = true,
                "--rotate-180" => cfg.rotate_180 = true,
                "--portrait-ccw" => cfg.portrait_ccw = true,
                "--orientation" => {
                    let raw = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--orientation needs a value"))?;
                    cfg.orientation = Orientation::parse(&raw).ok_or_else(|| {
                        anyhow::anyhow!("--orientation: '{raw}' must be auto|landscape|portrait")
                    })?;
                }
                "-h" | "--help" => {
                    print!("{HELP}");
                    std::process::exit(0);
                }
                "--display" => {
                    cfg.display_index = Some(parse_next(&mut args, "--display")?);
                }
                "--width" => cfg.width = parse_next(&mut args, "--width")?,
                "--height" => cfg.height = parse_next(&mut args, "--height")?,
                "--fps" => {
                    let v: u32 = parse_next(&mut args, "--fps")?;
                    if v == 0 {
                        bail!("--fps must be >= 1");
                    }
                    cfg.fps = v as f32;
                }
                "--quality" => {
                    let v: u8 = parse_next(&mut args, "--quality")?;
                    if !(1..=100).contains(&v) {
                        bail!("--quality must be 1-100");
                    }
                    cfg.quality = v;
                }
                "--keepalive-ms" => {
                    cfg.keepalive_ms = parse_next(&mut args, "--keepalive-ms")?;
                }
                other => bail!("unknown argument: {other}\n\n{HELP}"),
            }
        }

        Ok(cfg)
    }
}

fn parse_next<T: std::str::FromStr, I: Iterator<Item = String>>(
    args: &mut I,
    flag: &str,
) -> Result<T> {
    let raw = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("{flag} needs a value"))?;
    raw.parse::<T>()
        .map_err(|_| anyhow::anyhow!("{flag}: '{raw}' is not valid"))
}
