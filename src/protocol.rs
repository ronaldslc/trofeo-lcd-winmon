//! Clean-room implementation of the Thermalright "LY" / TrofeoBulk USB bulk protocol.
//!
//! See `docs/PROTOCOL.md` for the independent wire specification this module implements.

use crate::error::TrofeoError;

/// USB vendor ID for the Trofeo Vision family.
pub const VENDOR_ID: u16 = 0x0416;
/// Trofeo Vision 9.16" / 11.3" (LY variant).
pub const PID_LY: u16 = 0x5408;
/// Trofeo Vision LY1 variant.
pub const PID_LY1: u16 = 0x5409;

/// 16-byte handshake header; the write is padded with zeros to 2048 bytes.
pub const HANDSHAKE_HEADER: [u8; 16] = [
    0x02, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
pub const HANDSHAKE_ZERO_PAD: usize = 2032;
pub const HANDSHAKE_WRITE_LEN: usize = 16 + HANDSHAKE_ZERO_PAD;
pub const HANDSHAKE_READ_LEN: usize = 512;

pub const CHUNK_SIZE: usize = 512;
pub const CHUNK_HEADER_LEN: usize = 16;
pub const CHUNK_DATA_LEN: usize = 496;
pub const BULK_WRITE_LEN: usize = 4096;

/// Firmware cap on a single JPEG frame.
pub const MAX_JPEG_BYTES: usize = 450_000;

/// Device protocol variant, derived from the USB product ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// PID 0x5408 — chunk command 1, chunk count padded to a multiple of 4.
    Ly,
    /// PID 0x5409 — chunk command 2, no padding.
    Ly1,
}

impl Variant {
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            PID_LY => Some(Variant::Ly),
            PID_LY1 => Some(Variant::Ly1),
            _ => None,
        }
    }

    /// Chunk header byte 8.
    pub fn chunk_cmd(self) -> u8 {
        match self {
            Variant::Ly => 1,
            Variant::Ly1 => 2,
        }
    }

    /// Chunk count is rounded up to this multiple for LY.
    pub fn chunk_pad_multiple(self) -> usize {
        match self {
            Variant::Ly => 4,
            Variant::Ly1 => 1,
        }
    }
}

/// Parsed handshake response.
#[derive(Debug, Clone)]
pub struct Handshake {
    /// Panel parameter byte returned by the firmware.
    pub pm: u8,
    /// Panel sub-parameter byte.
    pub sub: u8,
    /// Raw 512-byte response (kept for diagnostics / 11.3" detection).
    pub raw: Vec<u8>,
}

impl Handshake {
    /// Parse and validate a handshake response.
    pub fn parse(variant: Variant, response: &[u8]) -> Result<Self, TrofeoError> {
        if response.len() < 37 || response[0] != 0x03 || response[1] != 0xFF || response[8] != 0x01
        {
            return Err(TrofeoError::BadHandshake(response.to_vec()));
        }

        let (pm, sub) = match variant {
            Variant::Ly => {
                let mut raw = response[20];
                if raw <= 3 {
                    raw = 1;
                }
                (
                    64u8.wrapping_add(raw),
                    response.get(22).copied().unwrap_or(0).wrapping_add(1),
                )
            }
            Variant::Ly1 => (
                49u8.wrapping_add(response[20]),
                response.get(22).copied().unwrap_or(0),
            ),
        };

        Ok(Handshake {
            pm,
            sub,
            raw: response.to_vec(),
        })
    }

    /// Heuristic: InfoPanel identifies the 11.3" panel by `response[20] == 0x05`.
    pub fn is_trofeo_11_3(&self) -> bool {
        self.raw.get(20).copied() == Some(0x05)
    }
}

/// Build the chunked wire buffer for a JPEG payload.
///
/// Layout per 512-byte chunk:
/// `01 FF | total(u32 LE) | data_len(u16 LE) | cmd | num_chunks(u16 LE) | index(u16 LE) | pad | data`.
///
/// `num_chunks` is always `len / 496 + 1` (the protocol emits a short/empty terminator
/// chunk when the length is an exact multiple of 496).
pub fn build_chunks(variant: Variant, payload: &[u8]) -> Vec<u8> {
    let total = payload.len();
    let num_chunks = total / CHUNK_DATA_LEN + 1;
    let last_data = total % CHUNK_DATA_LEN;

    let mut out = vec![0u8; num_chunks * CHUNK_SIZE];
    for i in 0..num_chunks {
        let off = i * CHUNK_SIZE;
        let is_last = i + 1 == num_chunks;
        let data_len = if is_last { last_data } else { CHUNK_DATA_LEN };

        out[off] = 0x01;
        out[off + 1] = 0xFF;
        out[off + 2..off + 6].copy_from_slice(&(total as u32).to_le_bytes());
        out[off + 6..off + 8].copy_from_slice(&(data_len as u16).to_le_bytes());
        out[off + 8] = variant.chunk_cmd();
        out[off + 9..off + 11].copy_from_slice(&(num_chunks as u16).to_le_bytes());
        out[off + 11..off + 13].copy_from_slice(&(i as u16).to_le_bytes());

        let src = i * CHUNK_DATA_LEN;
        let dst = off + CHUNK_HEADER_LEN;
        out[dst..dst + data_len].copy_from_slice(&payload[src..src + data_len]);
    }

    // LY: pad the chunk count up to a multiple of 4 with zero bytes, so the total
    // buffer length is always a multiple of 2048.
    let multiple = variant.chunk_pad_multiple();
    let remainder = num_chunks % multiple;
    let padded = if remainder == 0 {
        num_chunks
    } else {
        num_chunks + (multiple - remainder)
    };
    out.resize(padded * CHUNK_SIZE, 0);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_yields_single_terminator_chunk() {
        let buf = build_chunks(Variant::Ly, &[]);
        assert_eq!(buf.len(), 4 * CHUNK_SIZE); // one chunk padded to a multiple of 4
        assert_eq!(buf[0], 0x01);
        assert_eq!(buf[1], 0xFF);
        assert_eq!(u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]), 0);
    }

    #[test]
    fn ly_buffer_is_multiple_of_2048() {
        for len in [1usize, 495, 496, 497, 1000, 100_000] {
            let payload = vec![0xAB; len];
            let buf = build_chunks(Variant::Ly, &payload);
            assert_eq!(buf.len() % 2048, 0, "len={len}");
        }
    }

    #[test]
    fn chunk_header_fields_are_correct() {
        let payload = vec![0x5A; 600];
        let buf = build_chunks(Variant::Ly, &payload);
        // num_chunks = 600/496 + 1 = 2
        assert_eq!(u16::from_le_bytes([buf[9], buf[10]]), 2);
        assert_eq!(buf[8], 1); // LY command
        assert_eq!(u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]), 600);
        // first chunk data_len = 496
        assert_eq!(u16::from_le_bytes([buf[6], buf[7]]), 496);
        // second chunk index = 1, data_len = 600 % 496 = 104
        let off = CHUNK_SIZE;
        assert_eq!(u16::from_le_bytes([buf[off + 11], buf[off + 12]]), 1);
        assert_eq!(u16::from_le_bytes([buf[off + 6], buf[off + 7]]), 104);
    }

    #[test]
    fn parse_rejects_garbage() {
        let bad = vec![0u8; 40];
        assert!(Handshake::parse(Variant::Ly, &bad).is_err());
    }
}
