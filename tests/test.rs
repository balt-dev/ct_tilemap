#![feature(const_float_bits_conv)]

use const_str::concat_bytes;
use std::io::Cursor;
use ct_tilemap::TileMap;

#[test]
fn round_trip_test() -> Result<(), Box<dyn std::error::Error>> {
    const FILE: &[u8] = concat_bytes!(
        b"ACHTUNG!", // Magic string
        b"\x05\x01", // Version 5
        b"MAP ", // Property mapping
            51_u32.to_le_bytes(), // Block length
            3_u16.to_le_bytes(), // Number of properties
            6, b"Integer", // Property 1
                0, 196_i32.to_le_bytes(), // Integer
            4, b"Float", // Property 2
                1, 2.2_f32.to_le_bytes(), // Float
            5, b"String", // Property 3
                2, 12_u32.to_le_bytes(), b"Hello, world!", // String
        b"TILE", // Tilesets
            32_u32.to_le_bytes(), // Block length
            2, // Number of tilesets
                0, 0xda, 0x89, 0x72, // xBGR color
                12, b"overworld.png",
                0, 0x3F, 0x39, 0x36, // xBGR color
                7, b"cave.png",
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
                2, // Two data blocks
                b"MAIN", // Main tile data
                    // Compressed data
                    31_u32.to_le_bytes(), // Length
                    0x78, 0x9c, 0x4d, 0xc9, 0x01, 0x06, 0x00, 0x00,
                    0x10, 0x02, 0xc1, 0xfd, 0xff, 0xa7, 0x37, 0x27,
                    0x71, 0x43, 0x94, 0x14, 0x9a, 0xdf, 0x66, 0xab,
                    0xcf, 0xd1, 0x00, 0x55, 0x07, 0x1f, 0xe1,
                b"DATA",
                    1, // Cell size
                    [0x00, 0x00, 0x00, 0x00], // Default value
                    // Compressed data
                        27_u32.to_le_bytes(), // Length
                        0x78, 0x9c, 0x25, 0xc4, 0x81, 0x09, 0x00, 0x00,
                        0x10, 0x82, 0xc0, 0x72, 0xff, 0xa1, 0xfb, 0x47,
                        0x85, 0x4b, 0x6f, 0xf9, 0x50, 0xc8, 0x00, 0x00,
                        0x91, 0x00, 0x0d
    );
    let file = Cursor::new(FILE);
    let map = TileMap::read(file)?;
    let mut buf = Cursor::new(Vec::new());
    map.write(&mut buf)?;
    buf.set_position(0);
    let same_map = TileMap::read(buf)?;
    assert_eq!(map, same_map, "Round-trip test failed!");
    Ok(())
}