//! WinUSB access to the Trofeo Vision panel via `rusb` (libusb).

use std::time::Duration;

use rusb::{Context, DeviceHandle, Direction, TransferType, UsbContext};

use crate::error::TrofeoError;
use crate::protocol::{self, Handshake, Variant};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(1000);
const WRITE_TIMEOUT: Duration = Duration::from_millis(5000);
const READ_TIMEOUT: Duration = Duration::from_millis(1000);

/// An opened + claimed Trofeo Vision USB device.
pub struct TrofeoDevice {
    handle: DeviceHandle<Context>,
    interface: u8,
    ep_out: u8,
    ep_in: u8,
    variant: Variant,
}

impl TrofeoDevice {
    /// Find and open the first LY/LY1 device, claim its bulk interface.
    pub fn open() -> Result<Self, TrofeoError> {
        let context = Context::new()?;

        for device in context.devices()?.iter() {
            let descriptor = device.device_descriptor()?;
            if descriptor.vendor_id() != protocol::VENDOR_ID {
                continue;
            }
            let Some(variant) = Variant::from_pid(descriptor.product_id()) else {
                continue;
            };

            let handle = device.open()?;
            let config = device.active_config_descriptor()?;

            // Prefer an interface that exposes a bulk OUT + bulk IN pair.
            let mut found: Option<(u8, u8, u8)> = None;
            'search: for interface in config.interfaces() {
                for interface_desc in interface.descriptors() {
                    let mut ep_out = None;
                    let mut ep_in = None;
                    for endpoint in interface_desc.endpoint_descriptors() {
                        if endpoint.transfer_type() != TransferType::Bulk {
                            continue;
                        }
                        match endpoint.direction() {
                            Direction::Out => ep_out = Some(endpoint.address()),
                            Direction::In => ep_in = Some(endpoint.address()),
                        }
                    }
                    if let (Some(out), Some(inp)) = (ep_out, ep_in) {
                        found = Some((interface.number(), out, inp));
                        break 'search;
                    }
                }
            }

            let (interface, ep_out, ep_in) = found.ok_or(TrofeoError::NoBulkEndpoints)?;

            // Best-effort detach of any kernel/other driver holding the interface.
            let _ = handle.set_auto_detach_kernel_driver(true);
            if handle.kernel_driver_active(interface).unwrap_or(false) {
                let _ = handle.detach_kernel_driver(interface);
            }
            handle.claim_interface(interface)?;

            return Ok(Self {
                handle,
                interface,
                ep_out,
                ep_in,
                variant,
            });
        }

        Err(TrofeoError::DeviceNotFound)
    }

    pub fn variant(&self) -> Variant {
        self.variant
    }

    pub fn endpoints(&self) -> (u8, u8) {
        (self.ep_out, self.ep_in)
    }

    /// Perform the init handshake and parse PM/SUB.
    pub fn handshake(&self) -> Result<Handshake, TrofeoError> {
        let mut buffer = vec![0u8; protocol::HANDSHAKE_WRITE_LEN];
        buffer[..16].copy_from_slice(&protocol::HANDSHAKE_HEADER);

        self.handle
            .write_bulk(self.ep_out, &buffer, HANDSHAKE_TIMEOUT)?;

        let mut response = [0u8; protocol::HANDSHAKE_READ_LEN];
        let read = self
            .handle
            .read_bulk(self.ep_in, &mut response, HANDSHAKE_TIMEOUT)?;

        Handshake::parse(self.variant, &response[..read])
    }

    /// Chunk and send one JPEG frame, then consume the ACK.
    pub fn send_jpeg(&self, jpeg: &[u8]) -> Result<(), TrofeoError> {
        if jpeg.is_empty() {
            return Err(TrofeoError::EmptyFrame);
        }
        if jpeg.len() > protocol::MAX_JPEG_BYTES {
            return Err(TrofeoError::FrameTooLarge(jpeg.len()));
        }

        let frame = protocol::build_chunks(self.variant, jpeg);
        self.write_frame(&frame)
    }

    fn write_frame(&self, frame: &[u8]) -> Result<(), TrofeoError> {
        let total = frame.len();
        let mut position = 0usize;

        while position < total {
            let remaining = total - position;
            let write_len = if remaining >= protocol::BULK_WRITE_LEN {
                protocol::BULK_WRITE_LEN
            } else if self.variant == Variant::Ly {
                // LY frames are padded to a multiple of 2048, so the tail is 0 or 2048.
                remaining.min(2048)
            } else {
                remaining
            };

            self.handle.write_bulk(
                self.ep_out,
                &frame[position..position + write_len],
                WRITE_TIMEOUT,
            )?;
            position += protocol::BULK_WRITE_LEN;
        }

        // Consume the frame ACK.
        let mut ack = [0u8; protocol::HANDSHAKE_READ_LEN];
        self.handle.read_bulk(self.ep_in, &mut ack, READ_TIMEOUT)?;

        Ok(())
    }
}

impl Drop for TrofeoDevice {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.interface);
    }
}
