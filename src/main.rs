//! trofeo-lcd-winmon — clean-room second-monitor bridge for the Thermalright Trofeo Vision 11.3".

mod config;
mod error;
mod frame;
mod jpeg;
mod protocol;
mod sender;
mod usb;

#[cfg(windows)]
mod capture;

use anyhow::Result;
use config::Config;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cfg = Config::parse(std::env::args().skip(1))?;
    real(cfg)
}

#[cfg(windows)]
fn real(cfg: Config) -> Result<()> {
    use std::sync::Arc;
    use std::time::Duration;

    if cfg.list_displays {
        println!("Detected monitors:");
        for monitor in capture::list_monitors()? {
            let marker = match capture::classify(&cfg, monitor.width, monitor.height) {
                Some(mode) => format!("  <-- MATCH ({})", mode.label()),
                None => String::new(),
            };
            println!(
                "[{:>2}] {:<30} {:<16} {}x{} @{}Hz{}",
                monitor.index,
                monitor.name,
                monitor.device_name,
                monitor.width,
                monitor.height,
                monitor.refresh,
                marker
            );
        }
        return Ok(());
    }

    if cfg.hide_console {
        capture::hide_console();
    }

    let device = usb::TrofeoDevice::open()?;
    let (ep_out, ep_in) = device.endpoints();
    println!(
        "Device found: VID=0416 variant={:?} endpoints OUT=0x{ep_out:02x} IN=0x{ep_in:02x}",
        device.variant()
    );

    let handshake = device.handshake()?;
    println!(
        "Handshake: PM={} SUB={}{}",
        handshake.pm,
        handshake.sub,
        if handshake.is_trofeo_11_3() {
            " (11.3\" marker detected)"
        } else {
            ""
        }
    );

    if cfg.test_pattern {
        let bars = frame::RgbFrame::color_bars(cfg.width, cfg.height);
        let jpeg = encode_with_fallback(bars.as_slice(), bars.width(), bars.height(), cfg.quality)?;
        device.send_jpeg(&jpeg)?;
        println!(
            "Sent colour-bar test frame ({} bytes). If the panel shows bars, the USB path works.",
            jpeg.len()
        );
        return Ok(());
    }

    if cfg.probe_only {
        println!("Probe complete; not streaming (--probe-only).");
        return Ok(());
    }

    let slot = sender::new_slot();
    let sender = sender::spawn(
        device,
        Arc::clone(&slot),
        Duration::from_millis(cfg.keepalive_ms),
    );

    println!(
        "Streaming target {}x{} at up to {:.0} fps, base quality {}, keep-alive every {} ms ...",
        cfg.width, cfg.height, cfg.fps, cfg.quality, cfg.keepalive_ms
    );
    println!("Orientation is auto-detected from each frame; change the monitor mode freely.");
    println!("Ensure TRCC / InfoPanel are closed (only one owner of the USB device).");

    // Supervise: if the capture session ends (e.g. a monitor mode change), re-detect the
    // monitor and start again while a matching one still exists.
    let result = loop {
        let target = match capture::select_target(&cfg) {
            Ok(target) => target,
            Err(err) => break Err(err),
        };
        println!(
            "  capturing \"{}\" {}x{} ({}) -> rotate {} deg",
            target.monitor.name().unwrap_or_default(),
            target.monitor.width().unwrap_or(0),
            target.monitor.height().unwrap_or(0),
            target.mode.label(),
            target.rotation.degrees()
        );

        match capture::run(cfg.clone(), target, Arc::clone(&slot)) {
            Ok(()) => {
                eprintln!("capture session ended; re-detecting monitor ...");
                std::thread::sleep(Duration::from_millis(800));
            }
            Err(err) => break Err(err),
        }
    };

    sender.shutdown();
    result
}

/// Encode RGB as JPEG, reducing quality until it fits the firmware cap.
#[cfg(windows)]
fn encode_with_fallback(rgb: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>> {
    let mut quality = quality.clamp(1, 100);
    loop {
        let jpeg = jpeg::encode_rgb(rgb, width, height, quality)
            .map_err(|e| anyhow::anyhow!("JPEG encode failed: {e}"))?;
        if jpeg.len() <= protocol::MAX_JPEG_BYTES {
            return Ok(jpeg);
        }
        if quality <= 20 {
            anyhow::bail!(
                "JPEG stays above the 450000-byte cap at quality 20 (got {})",
                jpeg.len()
            );
        }
        quality = quality.saturating_sub(12).max(20);
    }
}

#[cfg(not(windows))]
fn real(_cfg: Config) -> Result<()> {
    anyhow::bail!("trofeo-lcd-winmon targets Windows only")
}
