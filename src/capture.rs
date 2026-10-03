//! Windows Graphics Capture glue: pick the virtual monitor (landscape or portrait),
//! capture it with the cursor, and publish encoded frames for the sender thread.

#![cfg(windows)]

use std::time::Duration;

use anyhow::{anyhow, Result};
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use crate::config::{Config, Orientation};
use crate::frame::{RgbFrame, Rotation};
use crate::protocol;
use crate::sender::{self, FrameSlot};

/// How the panel is mounted relative to the strip's native landscape pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Landscape,
    Portrait,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Landscape => "landscape",
            Mode::Portrait => "portrait",
        }
    }
}

/// A monitor as reported by Windows Graphics Capture.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub device_name: String,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

/// A chosen capture target plus the rotation needed to reach the panel's native pixels.
pub struct Target {
    pub monitor: Monitor,
    pub mode: Mode,
    pub rotation: Rotation,
}

pub fn list_monitors() -> Result<Vec<MonitorInfo>> {
    let monitors = Monitor::enumerate().map_err(|e| anyhow!("monitor enumerate: {e}"))?;
    let mut out = Vec::with_capacity(monitors.len());
    for (index, monitor) in monitors.iter().enumerate() {
        out.push(MonitorInfo {
            index,
            name: monitor.name().unwrap_or_default(),
            device_name: monitor.device_name().unwrap_or_default(),
            width: monitor.width().unwrap_or(0),
            height: monitor.height().unwrap_or(0),
            refresh: monitor.refresh_rate().unwrap_or(0),
        });
    }
    Ok(out)
}

/// Classify a monitor resolution as the panel's landscape size (`WxH`) or portrait
/// size (`HxW`) — or `None` if it is neither. This is the auto-detection core.
pub fn classify(cfg: &Config, width: u32, height: u32) -> Option<Mode> {
    if width == cfg.width && height == cfg.height {
        Some(Mode::Landscape)
    } else if width == cfg.height && height == cfg.width {
        Some(Mode::Portrait)
    } else {
        None
    }
}

/// Is this monitor allowed by the requested `--orientation`?
fn orientation_allows(cfg: &Config, width: u32, height: u32) -> bool {
    match cfg.orientation {
        Orientation::Auto => classify(cfg, width, height).is_some(),
        Orientation::Landscape => width == cfg.width && height == cfg.height,
        Orientation::Portrait => width == cfg.height && height == cfg.width,
    }
}

/// Rotation for a matching monitor: portrait needs 90° to reach the native landscape pixels.
fn base_rotation(cfg: &Config, mode: Mode) -> Rotation {
    match mode {
        Mode::Landscape => Rotation::R0,
        Mode::Portrait => {
            if cfg.portrait_ccw {
                Rotation::R270
            } else {
                Rotation::R90
            }
        }
    }
}

fn with_manual_180(cfg: &Config, rotation: Rotation) -> Rotation {
    if cfg.rotate_180 {
        rotation.add(Rotation::R180)
    } else {
        rotation
    }
}

/// Choose the capture monitor and the rotation to apply.
///
/// With `--orientation auto` (default) this accepts either a `WxH` (landscape) or an
/// `HxW` (portrait) monitor; with `--display N` the mode is derived from whatever that
/// monitor is.
pub fn select_target(cfg: &Config) -> Result<Target> {
    let monitors = Monitor::enumerate().map_err(|e| anyhow!("monitor enumerate: {e}"))?;

    if let Some(index) = cfg.display_index {
        let monitor = monitors
            .into_iter()
            .nth(index)
            .ok_or_else(|| anyhow!("no monitor with index {index}; run --list-displays"))?;
        let width = monitor.width().unwrap_or(0);
        let height = monitor.height().unwrap_or(0);
        let mode = classify(cfg, width, height).unwrap_or(if height > width {
            Mode::Portrait
        } else {
            Mode::Landscape
        });
        if classify(cfg, width, height).is_none() {
            eprintln!(
                "warning: monitor {index} is {}x{}, not {}x{} or {}x{}; it will be scaled to fit",
                width, height, cfg.width, cfg.height, cfg.height, cfg.width
            );
        }
        let rotation = with_manual_180(cfg, base_rotation(cfg, mode));
        return Ok(Target {
            monitor,
            mode,
            rotation,
        });
    }

    for monitor in monitors {
        let width = monitor.width().unwrap_or(0);
        let height = monitor.height().unwrap_or(0);
        if orientation_allows(cfg, width, height) {
            let mode = classify(cfg, width, height).unwrap_or(Mode::Landscape);
            let rotation = with_manual_180(cfg, base_rotation(cfg, mode));
            return Ok(Target {
                monitor,
                mode,
                rotation,
            });
        }
    }

    Err(anyhow!(
        "no {} monitor found ({}x{} or {}x{}); add one via vdd_settings.xml or pass --display; \
         run --list-displays",
        match cfg.orientation {
            Orientation::Auto => "matching",
            Orientation::Landscape => "landscape",
            Orientation::Portrait => "portrait",
        },
        cfg.width,
        cfg.height,
        cfg.height,
        cfg.width
    ))
}

/// Flags passed into the capture handler (must be `Send`).
pub struct BridgeFlags {
    pub config: Config,
    pub startup_mode: Mode,
    pub slot: FrameSlot,
}

struct Bridge {
    config: Config,
    startup_mode: Mode,
    slot: FrameSlot,
    /// Reused capture scratch buffer to avoid per-frame allocation.
    scratch: Vec<u8>,
    /// Last orientation seen, to log only on change.
    last_mode: Option<Mode>,
}

impl GraphicsCaptureApiHandler for Bridge {
    type Flags = BridgeFlags;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            config: ctx.flags.config,
            startup_mode: ctx.flags.startup_mode,
            slot: ctx.flags.slot,
            scratch: Vec::new(),
            last_mode: None,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        // 1. Capture -> RGB (borrow of `scratch` is scoped, so it can be reused).
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();

        let rgb = {
            let buffer = frame.buffer()?;
            let width = buffer.width();
            let height = buffer.height();
            let data = buffer.as_nopadding_buffer(&mut scratch);
            RgbFrame::from_bgra(data, width, height)
        };

        self.scratch = scratch;

        // 2. Detect orientation from the *live* frame so mode changes are picked up
        //    without a restart, rotate to the panel's native landscape, then scale.
        let mode = match self.config.orientation {
            Orientation::Auto => {
                classify(&self.config, rgb.width(), rgb.height()).unwrap_or(self.startup_mode)
            }
            Orientation::Landscape => Mode::Landscape,
            Orientation::Portrait => Mode::Portrait,
        };
        let rotation = with_manual_180(&self.config, base_rotation(&self.config, mode));

        if self.last_mode != Some(mode) {
            eprintln!(
                "orientation: {} (frame {}x{}) -> rotate {} deg",
                mode.label(),
                rgb.width(),
                rgb.height(),
                rotation.degrees()
            );
            self.last_mode = Some(mode);
        }

        let rgb = rotation.apply(&rgb);
        let rgb = if rgb.width() != self.config.width || rgb.height() != self.config.height {
            rgb.scaled_nearest(self.config.width, self.config.height)
        } else {
            rgb
        };

        // 3. Encode (reducing quality if needed) and publish for the sender thread.
        if let Ok(jpeg) = encode_with_fallback(&rgb, self.config.quality) {
            sender::publish(&self.slot, jpeg);
        }

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn encode_with_fallback(
    rgb: &RgbFrame,
    quality: u8,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut quality = quality.clamp(1, 100);
    loop {
        match crate::jpeg::encode_rgb(rgb.as_slice(), rgb.width(), rgb.height(), quality) {
            Ok(jpeg) if jpeg.len() <= protocol::MAX_JPEG_BYTES => return Ok(jpeg),
            Ok(jpeg) => {
                if quality <= 20 {
                    return Err(format!(
                        "JPEG stays above the 450000-byte cap at quality 20 (got {})",
                        jpeg.len()
                    )
                    .into());
                }
                quality = quality.saturating_sub(12).max(20);
            }
            Err(err) => return Err(format!("JPEG encode failed: {err}").into()),
        }
    }
}

/// Hide the console window (for autostart). Best-effort; safe to call in a console app.
pub fn hide_console() {
    use windows::Win32::System::Console::GetConsoleWindow;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    unsafe {
        let hwnd = GetConsoleWindow();
        if !hwnd.is_invalid() {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

/// Start streaming the given target. Blocks until capture stops.
pub fn run(cfg: Config, target: Target, slot: FrameSlot) -> Result<()> {
    let interval = Duration::from_millis((1000.0 / cfg.fps.max(1.0)) as u64);

    let settings = Settings::new(
        target.monitor,
        CursorCaptureSettings::WithCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Custom(interval),
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        BridgeFlags {
            config: cfg,
            startup_mode: target.mode,
            slot,
        },
    );

    Bridge::start(settings).map_err(|e| anyhow!("capture failed: {e}"))?;
    Ok(())
}
