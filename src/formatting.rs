use crate::{Layer, Property, SubLayer, Tile, TileMap, TileSet};
use fmt::Debug;
use std::fmt;
use std::fmt::{Display, Formatter, Write};

impl Debug for Layer {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if f.alternate() {
            writeln!(f, "Layer {{")?;
            let mut buf = String::new();
            write!(buf, "data: [")?;
            // Write data
            let w = self.width as usize;
            let h = self.height as usize;
            if w == 0 || h == 0 {
                writeln!(buf, "]\n,")?;
            } else {
                for (i, tile) in self.data.as_slice().iter().enumerate() {
                    if i % w == 0 {
                        // Write newline and padding
                        write!(buf, "\n    ")?;
                    }
                    // Write cell
                    write!(buf, "0x{tile}")?;
                    // Check if at end
                    if !(i % w == (w - 1) && i / w == h - 1) {
                        write!(buf, ", ")?;
                    }
                }
                writeln!(buf, "\n],")?;
            }
            writeln!(buf, "width: {:?},", self.width)?;
            writeln!(buf, "height: {:?},", self.height)?;
            writeln!(buf, "tileset: {:?},", self.tileset)?;
            writeln!(buf, "collision: {:?},", self.collision)?;
            writeln!(buf, "offset: {:?},", self.offset)?;
            writeln!(buf, "scroll: {:?},", self.scroll)?;
            writeln!(buf, "wrap: {:?},", self.wrap)?;
            writeln!(buf, "visible: {:?},", self.visible)?;
            writeln!(buf, "opacity: {:?},", self.opacity)?;
            writeln!(buf, "tile_dimensions: {:?},", self.tile_dimensions)?;
            writeln!(buf, "sublayer_link: {:?},", self.sublayer_link)?;
            writeln!(buf, "sublayers: {:#?},", self.sublayers)?;
            // Pad lines
            for line in buf.lines() {
                writeln!(f, "    {line}")?;
            }
            write!(f, "}}")
        } else {
            write!(f, "Layer {{ ")?;
            write!(f, "data: {:?}, ", self.data)?;
            write!(f, "width: {:?}, ", self.width)?;
            write!(f, "height: {:?}, ", self.height)?;
            write!(f, "tileset: {:?}, ", self.tileset)?;
            write!(f, "collision: {:?}, ", self.collision)?;
            write!(f, "offset: {:?}, ", self.offset)?;
            write!(f, "scroll: {:?}, ", self.scroll)?;
            write!(f, "wrap: {:?}, ", self.wrap)?;
            write!(f, "visible: {:?}, ", self.visible)?;
            write!(f, "opacity: {:?}, ", self.opacity)?;
            write!(f, "tile_dimensions: {:?}, ", self.tile_dimensions)?;
            write!(f, "sublayer_link: {:?}, ", self.sublayer_link)?;
            write!(f, "sublayers: {:?} }}", self.sublayers)
        }
    }
}

impl Debug for Property {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Property::Integer(i) => write!(fmt, "{i}"),
            Property::Float(f) => write!(fmt, "{f:?}"),
            Property::String(b) => {
                let escaped = b
                    .iter()
                    .copied()
                    .flat_map(std::ascii::escape_default)
                    .collect::<Vec<u8>>();
                write!(fmt, "\"{}\"", unsafe {
                    // SAFETY: escape_default always
                    // gives back valid UTF-8
                    String::from_utf8_unchecked(escaped)
                })
            }
        }
    }
}

impl Display for Tile {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:#?}")
    }
}

impl Debug for Tile {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // SAFETY: This is always valid due to every union element having the same size.
        if f.alternate() {
            // Write only ID in pretty print
            write!(f, "{:04X}", unsafe { self.id })
        } else {
            write!(f, "Tile({:04X})", unsafe { self.id })
        }
    }
}

impl Debug for SubLayer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            f.write_str("SubLayer {\n    data: [")?;
            // Write each cell, space-separated
            let chunk_size = self.cell_size as usize;
            let w = self.width as usize;
            let h = self.height as usize;
            if w == 0 {
                f.write_str("],\n")?;
            } else {
                for (i, cell) in self.data.as_slice().chunks(chunk_size).enumerate() {
                    if i % w == 0 {
                        // Write newline and padding
                        f.write_str("\n        ")?;
                    }
                    f.write_str("0x")?;
                    // Write cell
                    for byte in cell {
                        write!(f, "{byte:02X}")?;
                    }
                    // Check if at end
                    if !(i % w == (w - 1) && i / w == h - 1) {
                        f.write_str(", ")?;
                    }
                }
                f.write_str("\n    ],")?;
            }
            f.write_str("\n    default_value: 0x")?;
            for byte in &self.default_value[..self.cell_size as usize] {
                write!(f, "{byte:02X}")?;
            }
            write!(
                f,
                ",\n    width: {},\n    height: {}\n}}",
                self.width, self.height
            )
        } else {
            write!(
                f, "SubLayer{{ data: {:02X?}, default_value: {:02X?}, cell_size: {}, width: {}, height: {} }}",
                self.data,
                self.default_value,
                self.cell_size,
                self.width,
                self.height
            )
        }
    }
}

impl Debug for TileMap {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "TileMap {{")?;
        let mut buf = String::new();
        writeln!(buf, "layers: {:#?},", self.layers)?;
        writeln!(buf, "tilesets: {:#?},", self.tilesets)?;
        writeln!(buf, "properties: {:#?}", self.properties)?;
        // Pad lines
        for line in buf.lines() {
            writeln!(f, "    {line}")?;
        }
        write!(f, "}}")
    }
}

impl Debug for TileSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(
                f,
                "TileSet {{\n    path: {:#?},\n    transparent_color: #{:02X}{:02X}{:02X}\n}}",
                self.path,
                self.transparent_color.0,
                self.transparent_color.1,
                self.transparent_color.2
            )
        } else {
            write!(
                f,
                "TileSet {{ path: {:?}, transparent_color: {:?} }}",
                self.path, self.transparent_color
            )
        }
    }
}
