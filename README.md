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

## License

Licensed under either of

* Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
