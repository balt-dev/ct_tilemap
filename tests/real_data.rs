use ct_tilemap::TileMap;

// This file is a custom level (that I made) from Baba Is You.
const FILE: &[u8] = include_bytes!("real_data.l");

#[test]
fn real_data() {
    let t = TileMap::read(FILE).expect("failed to read file");
    eprintln!("{t:#?}");
}