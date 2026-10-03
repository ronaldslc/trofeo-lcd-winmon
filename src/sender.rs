//! Frame delivery to the panel.
//!
//! Windows Graphics Capture only produces frames when the desktop *changes*. The panel
//! firmware reverts to its built-in logo when it stops receiving frames, so a dedicated
//! sender thread owns the USB device and:
//!
//! * sends a freshly captured frame as soon as it is published, and
//! * re-sends the last frame every `keepalive` interval when the desktop is static.
//!
//! Capture never touches USB directly, so a slow USB write cannot stall the capturer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::usb::TrofeoDevice;

#[derive(Default)]
pub struct FrameSlotInner {
    jpeg: Option<Arc<Vec<u8>>>,
    version: u64,
}

/// Shared "latest encoded frame" mailbox between the capturer and the sender thread.
pub type FrameSlot = Arc<Mutex<FrameSlotInner>>;

pub fn new_slot() -> FrameSlot {
    Arc::new(Mutex::new(FrameSlotInner::default()))
}

/// Publish a freshly encoded JPEG as the newest frame.
pub fn publish(slot: &FrameSlot, jpeg: Vec<u8>) {
    if let Ok(mut inner) = slot.lock() {
        inner.jpeg = Some(Arc::new(jpeg));
        inner.version = inner.version.wrapping_add(1);
    }
}

pub struct SenderHandle {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl SenderHandle {
    /// Ask the sender thread to stop and wait for it to finish.
    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Start the sender thread. It takes ownership of the opened USB device.
pub fn spawn(device: TrofeoDevice, slot: FrameSlot, keepalive: Duration) -> SenderHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);

    let join = thread::spawn(move || {
        let mut last_version = u64::MAX;
        // Force an immediate send once a frame exists.
        let mut last_sent = Instant::now() - keepalive;

        while !stop_thread.load(Ordering::Relaxed) {
            let newest = {
                match slot.lock() {
                    Ok(inner) => inner.jpeg.as_ref().map(|j| (Arc::clone(j), inner.version)),
                    Err(_) => None,
                }
            };

            if let Some((jpeg, version)) = newest {
                let changed = version != last_version;
                let keepalive_due = last_sent.elapsed() >= keepalive;

                if changed || keepalive_due {
                    match device.send_jpeg(&jpeg) {
                        Ok(()) => {
                            last_version = version;
                            last_sent = Instant::now();
                        }
                        Err(err) => eprintln!("USB send failed: {err}"),
                    }
                }
            }

            thread::sleep(Duration::from_millis(5));
        }
    });

    SenderHandle {
        stop,
        join: Some(join),
    }
}
