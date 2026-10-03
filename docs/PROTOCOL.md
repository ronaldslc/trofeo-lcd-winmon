# Clean-room wire specification — "LY" / TrofeoBulk USB protocol

This is a functional, independently written description of the wire format needed to drive
the panel. It is derived from the public reverse-engineering documents referenced at the
bottom; it is **not** copied code. Where a byte is a "magic" value it is stated as a fact.

## Transport

- USB, vendor-specific device, WinUSB bulk.
- Vendor ID `0x0416`.
- Product ID `0x5408` (LY) or `0x5409` (LY1).
- Endpoints: use an interface that exposes a **bulk OUT + bulk IN pair**. Endpoint addresses
  are not hard-coded; discover them from the configuration descriptor and prefer an interface
  with both bulk directions.
- Typical endpoint address for this device: `0x01` (OUT) / `0x81` (IN), observe `0x09` on
  some units/firmware.

## Constants

| Name | Value |
|---|---|
| `HANDSHAKE_HEADER` | `02 FF 00 00 00 00 00 00 01 00 00 00 00 00 00 00` |
| Handshake write length | 16 + 2032 = **2048** bytes |
| Handshake read length | **512** bytes |
| Chunk size (wire) | **512** bytes |
| Chunk header size | **16** bytes |
| Chunk payload size | **496** bytes |
| Bulk write size | **4096** bytes |
| Max JPEG frame | **450000** bytes |

## 1. Handshake

1. Build a 2048-byte buffer: the 16-byte `HANDSHAKE_HEADER` followed by 2032 zero bytes.
2. `write_bulk(out_ep, buffer)`.
3. `read_bulk(in_ep, 512)`.
4. Validate: `response[0] == 0x03 && response[1] == 0xFF && response[8] == 0x01`.
   If invalid, abort (bad handshake).
5. Extract panel parameters:
   - **LY (`0x5408`)**:
     - `raw = response[20]`; if `raw <= 3` then `raw = 1`.
     - `PM = 64 + raw`.
     - `SUB = response[22] + 1`.
   - **LY1 (`0x5409`)**:
     - `PM = 49 + response[20]`.
     - `SUB = response[22]`.

Observed `PM` values (informational):

| Panel | PM |
|---|---|
| Trofeo Vision 9.16" | 65 |
| Trofeo Vision 11.3" | expected distinct value from the 11.3" marker (InfoPanel keys on `response[20] == 0x05`, i.e. `PM = 69` under the LY formula) |

The 11.3" firmware reports `1920x480` in its own device report, but the physical panel is
`1920x400`; we always render `1920x400`.

## 2. Frame chunking

Given a JPEG payload of `N` bytes (must be `<= 450000`):

```
num_chunks = N / 496 + 1            // NOTE: always +1, even when N is a multiple of 496
last_data  = N % 496
```

Each chunk is 512 bytes:

| Offset | Size | Meaning |
|---|---|---|
| 0 | 1 | `0x01` (start magic) |
| 1 | 1 | `0xFF` |
| 2 | 4 | `N` (total payload size), little-endian u32 |
| 6 | 2 | `data_len` for this chunk, little-endian u16 (496, or `last_data` for the last chunk) |
| 8 | 1 | chunk command: `1` for LY, `2` for LY1 |
| 9 | 2 | `num_chunks`, little-endian u16 |
| 11 | 2 | chunk index `i`, little-endian u16 |
| 13 | 3 | zero padding |
| 16 | up to 496 | payload bytes (zero-filled for the short last chunk) |

After building `num_chunks`, **LY** pads the buffer with zero bytes up to a chunk count
that is a multiple of **4** (so the total byte length is a multiple of 2048). **LY1** does
not pad.

## 3. Sending chunks

Write the chunk buffer in `4096`-byte slices:

- Iterate `pos` from `0`, writing `min(4096, remaining)` bytes, then `pos += 4096`.
- For **LY**, the final remainder is capped at `2048` (the multiple-of-4 padding above
  guarantees the tail is 0 or 2048).
- After the write loop, `read_bulk(in_ep, 512)` to consume the ACK.

## 4. Frame content

The payload is a **JPEG** (baseline) image. Nominal frame size = the panel's native
resolution, `1920x400` for the 11.3". Encoders should:

- Keep quality such that the encoded size stays under 450,000 bytes.
- Reduce quality and re-encode if the limit is exceeded (recommended floor ~20).

## 5. Timing / cadence

- The panel accepts discrete JPEG frames; there is no vsync. Practical throughput on USB 2.0
  bulk is roughly 10-30 fps at this resolution.
- No frame needs to be sent while the desktop is static.

## References (protocol provenance)

- https://github.com/haven80/trofeo-lcd-rs (`src/lib.rs`: constants, `build_chunks`, handshake)
- https://github.com/Lexonight1/thermalright-trcc-linux (Python `LyLcd`)
- https://github.com/sukualam/trofeo-lcd
- https://github.com/emaspa/infopanel-1 (`InfoPanel/ThermalrightPanel/*`, `PANELS.md`)
