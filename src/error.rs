//! Error type shared by the USB/protocol layers.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TrofeoError {
    #[error("Trofeo device (0416:5408 / 0416:5409) not found")]
    DeviceNotFound,

    #[error("no interface with a bulk OUT + bulk IN endpoint pair was found")]
    NoBulkEndpoints,

    #[error("USB error: {0}")]
    Usb(#[from] rusb::Error),

    #[error("bad handshake response: {0:02x?}")]
    BadHandshake(Vec<u8>),

    #[error("empty JPEG frame")]
    EmptyFrame,

    #[error("JPEG frame of {0} bytes exceeds the firmware limit of 450000 bytes")]
    FrameTooLarge(usize),
}
