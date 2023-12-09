Simple library to handle [Clickteam TileMap](https://github.com/clickteam-plugin/TileMap) files.

```rust
use ct_tilemap::{TileMap, Tile};

fn main() {
    let mut tilemap = TileMap::read(
        /* .. */
    )?;

    for layer in tilemap.layers.iter_mut() {
        layer.resize(8, 8);
        layer[(0, 0)] = Tile {id: 0x1234};
        layer[(0, 1)] = Tile {position: [5, 3]};
        let sublayer = layer.add_sublayer(b"YES");
        sublayer[(3, 3)].copy_from_slice(b"NO!");
    }

    tilemap.write(
        /* .. */
    )?;
}
```