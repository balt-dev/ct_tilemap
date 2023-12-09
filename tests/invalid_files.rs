#![feature(const_float_bits_conv)]

use std::io;
use const_str::concat_bytes;
use std::io::Read;
use ct_tilemap::{ReadError, TileMap};

struct ErrorsAtEnd(usize, &'static [u8]);
impl Read for ErrorsAtEnd {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        if self.1.len() < self.0 + b.len() {
            Err(io::Error::other("oh no!"))
        } else {
            b.copy_from_slice(&self.1[self.0 .. self.0 + b.len()]);
            self.0 += b.len();
            Ok(b.len())
        }
    }
}

const WRONG_STRING: &[u8] = b"INVALID!";
const UNSUPPORTED_VERSION: &[u8] = concat_bytes!(
    b"ACHTUNG!", // Magic string
    b"\x09\x01", // Version 9
);

const INVALID_HEADER: &[u8] = concat_bytes!(
    b"ACHTUNG!", // Magic string
    b"\x05\x01", // Version 5
    b"OHNO\x00\x00\x00\x00",
);

const INVALID_MAPPING: &[u8] = concat_bytes!(
    b"ACHTUNG!", // Magic string
    b"\x05\x01", // Version 5
    b"MAP ", // Property mapping
        27_u32.to_le_bytes(), // Block length
        3_u16.to_le_bytes(), // Number of properties
        6, b"Invalid", // Property 1
            9, b"Invalid property"
);

const INVALID_LAYER_SIZE: &[u8] = concat_bytes!(
    b"ACHTUNG!", // Magic string
    b"\x05\x01", // Version 5
    b"LAYR", // Layers
        128_u32.to_le_bytes(), // Block length
        1_u16.to_le_bytes(), // Number of layers
            5_u32.to_le_bytes(), 5_u32.to_le_bytes(), // Dimensions
            8_u16.to_le_bytes(), 8_u16.to_le_bytes(), // Tile dimensions
            0xFF, 0xFF, // Tileset and collision
            0_u32.to_le_bytes(), 0_u32.to_le_bytes(), // Offset
            0_f32.to_le_bytes(), 0_f32.to_le_bytes(), // Scroll
            0, 0, // Wrap,
            1, // Visible,
            0.9_f32.to_le_bytes(), // Opacity,
            0xFF, 0xFF, 0xFF, // Sublayer stuff
            // Data blocks
            1, // One data block
            b"MAIN", // Main tile data
                // Compressed data
                23_u32.to_le_bytes(), // Length
                0x78, 0x9c, 0x05, 0x80, 0xa1, 0x09, 0x00, 0x00,
                0x00, 0xc2, 0xde, 0xb5, 0x09, 0xd3, 0xf7, 0xc3,
                0x58, 0xca, 0x05, 0x06, 0x89, 0x02, 0x31
);

const INVALID_LAYER_HEADER: &[u8] = concat_bytes!(
    b"ACHTUNG!", // Magic string
    b"\x05\x01", // Version 5
    b"LAYR", // Layers
        128_u32.to_le_bytes(), // Block length
        1_u16.to_le_bytes(), // Number of layers
            5_u32.to_le_bytes(), 5_u32.to_le_bytes(), // Dimensions
            8_u16.to_le_bytes(), 8_u16.to_le_bytes(), // Tile dimensions
            0xFF, 0xFF, // Tileset and collision
            0_u32.to_le_bytes(), 0_u32.to_le_bytes(), // Offset
            0_f32.to_le_bytes(), 0_f32.to_le_bytes(), // Scroll
            0, 0, // Wrap,
            1, // Visible,
            0.9_f32.to_le_bytes(), // Opacity,
            0xFF, 0xFF, 0xFF, // Sublayer stuff
            // Data blocks
            1, // One data block
            b"OHNO"
);

const INVALID_COMPRESSED: &[u8] = concat_bytes!(
    b"ACHTUNG!", // Magic string
    b"\x05\x01", // Version 5
    b"LAYR", // Layers
        128_u32.to_le_bytes(), // Block length
        1_u16.to_le_bytes(), // Number of layers
            5_u32.to_le_bytes(), 5_u32.to_le_bytes(), // Dimensions
            8_u16.to_le_bytes(), 8_u16.to_le_bytes(), // Tile dimensions
            0xFF, 0xFF, // Tileset and collision
            0_u32.to_le_bytes(), 0_u32.to_le_bytes(), // Offset
            0_f32.to_le_bytes(), 0_f32.to_le_bytes(), // Scroll
            0, 0, // Wrap,
            1, // Visible,
            0.9_f32.to_le_bytes(), // Opacity,
            0xFF, 0xFF, 0xFF, // Sublayer stuff
            // Data blocks
            1, // One data block
            b"MAIN", // Main tile data
                // Compressed data
                5_u32.to_le_bytes(), // Length
                0x00, 0x00, 0x00, 0x00, 0x00
);

#[test]
fn invalid_files() {
    assert!(matches!(dbg!(TileMap::read(WRONG_STRING)).unwrap_err(), ReadError::InvalidMagic));
    assert!(matches!(dbg!(TileMap::read(UNSUPPORTED_VERSION)).unwrap_err(), ReadError::UnsupportedVersion(9)));
    assert!(matches!(dbg!(TileMap::read(INVALID_HEADER)).unwrap_err(), ReadError::InvalidHeader(_)));
    assert!(matches!(dbg!(TileMap::read(INVALID_LAYER_HEADER)).unwrap_err(), ReadError::InvalidHeader(_)));
    assert!(matches!(dbg!(TileMap::read(INVALID_MAPPING)).unwrap_err(), ReadError::InvalidType(9)));
    assert!(matches!(dbg!(TileMap::read(INVALID_LAYER_SIZE)).unwrap_err(), ReadError::InvalidLayerLength));
    assert!(matches!(dbg!(TileMap::read(INVALID_COMPRESSED)).unwrap_err(), ReadError::IoError(_)));
    assert!(matches!(dbg!(TileMap::read(ErrorsAtEnd(0, b"ACHTUNG!\x05\x01"))).unwrap_err(), ReadError::IoError(_)));
}