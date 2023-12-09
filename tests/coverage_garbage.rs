#![feature(const_float_bits_conv)]

use const_str::concat_bytes;
use std::io::Cursor;
use ct_tilemap::{Property, Tile, TileMap};

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

#[test]
fn round_trip_test() -> Result<(), Box<dyn std::error::Error>> {
    let file = Cursor::new(FILE);
    let map = TileMap::read(file)?;
    let mut buf = Cursor::new(Vec::new());
    map.write(&mut buf)?;
    buf.set_position(0);
    let same_map = TileMap::read(buf)?;
    assert_eq!(map, same_map, "Round-trip test failed!");

    let file = Cursor::new(FILE);
    let mut tilemap = TileMap::read(file)?;
    eprintln!("{tilemap:?}");
    for layer in &mut tilemap.layers {
        // This sequence covers all cases of resize
        let _ = layer.add_sublayer(b"YES");
        layer.resize(0, 0);
        layer.resize(8, 8);
        layer.resize(8, 8);
        layer.resize(8, 7);
        layer.resize(7, 8);
        layer.resize(7, 7);
        layer.resize(7, 8);
        layer.resize(8, 7);
        layer.resize(8, 8);
        assert_eq!(layer.width(), 8);
        assert_eq!(layer.height(), 8);
        // Test setting
        layer[(0, 0)] = Tile {id: 0x1234};
        *layer.get_mut((0, 1)).unwrap() = Tile {position: [5, 3]};
        assert_eq!(layer.get((2, 2)).unwrap(), &layer[(2, 2)]);
        let mut set_tile = layer[(0, 0)];
        assert_eq!(set_tile.id(), 0x1234);
        if cfg!(target_endian = "big") {
            assert_eq!(set_tile.position(), [0x12, 0x34]);
        } else {
            assert_eq!(set_tile.position(), [0x34, 0x12]);
        }
        *set_tile.id_mut() = 0x4321;
        *set_tile.position_mut() = [0xAB, 0xCD];
        if cfg!(target_endian = "big") {
            assert_eq!(set_tile.id(), 0xABCD);
        } else {
            assert_eq!(set_tile.id(), 0xCDAB);
        }
        assert!(layer.get((9, 9)).is_none());
        assert!(layer.get((5, 5)).is_some());
        assert!(layer.get_mut((9, 9)).is_none());
        assert!(layer.get_mut((5, 5)).is_some());
        let sublayer = layer.sublayers.last_mut().unwrap();
        assert_eq!(sublayer.width(), 8);
        assert_eq!(sublayer.height(), 8);
        assert_eq!(sublayer.cell_size(), 3);
        sublayer.set_default(b"YES!"); // Small -> big
        sublayer.set_default(b"Y"); // Big -> small
        sublayer.set_default(b""); // Small -> 0
        sublayer.set_default(b"YES!!!!"); // 0 -> Big
        assert_eq!(sublayer.cell_size(), 4); // Should get truncated
        sublayer[(3, 3)].copy_from_slice(b"NO! ");
        assert_eq!(&sublayer[(3, 4)], &[0; 4]); // This got nulled by setting cell size to 0
        assert!(sublayer.get((9, 9)).is_none());
        assert!(sublayer.get((5, 5)).is_some());
        assert!(sublayer.get_mut((9, 9)).is_none());
        assert!(sublayer.get_mut((5, 5)).is_some());
        eprintln!("{layer:#?}");
        eprintln!("{layer:?}");
    }

    assert_eq!(Property::from(1), Property::Integer(1));
    assert_eq!(Property::from(2.5), Property::Float(2.5));
    assert_eq!(Property::from(b"Yo!".to_vec()), Property::String(b"Yo!".to_vec()));
    assert_eq!(Property::from("Hi!".to_string()), Property::String(b"Hi!".to_vec()));
    Ok(())
}
