#![warn(missing_docs)]
#![warn(clippy::pedantic, clippy::perf, clippy::cargo)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::cast_lossless,
    clippy::comparison_chain  // according to their docs, this is not always optimized reliably
)]

/*!
Simple library to handle [Clickteam TileMap](https://github.com/clickteam-plugin/TileMap) files.

```rust
# use std::io::Cursor; use ct_tilemap::{Tile, TileMap, ReadError};
# fn main() -> Result<(), ReadError> {
#
# struct TrashWriter;
# impl std::io::Write for TrashWriter {
#   fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {Ok(buf.len())}
#   fn flush(&mut self) -> std::io::Result<()> {Ok(())}
# }
#
let mut tilemap = TileMap::read(
    /* .. */
     # Cursor::new(b"ACHTUNG!\x05\x01TILE\x00\x00\x00\x00\x01\x00\xda\x89\x72\x08tiles.png")
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
   # TrashWriter
)?;
#
# Ok(())
# }
```
 */

use bytemuck::{cast_slice, Pod, Zeroable};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fmt::{Display, Formatter};
use std::{
    collections::HashMap,
    io::{self, Cursor, Read, Write},
    iter,
    ops::{Index, IndexMut},
};

mod formatting;
mod read_helper;
mod write_helper;

/// A representation of a tilemap file.
#[derive(Clone, PartialEq, Default)]
pub struct TileMap {
    /// A collection of each layer of the tilemap.
    /// Any more than 65536 layers will not be saved.
    pub layers: Vec<Layer>,
    /// A collection of the tilesets of the tilemap.
    /// Any more than 256 tilesets will not be saved.
    pub tilesets: Vec<TileSet>,
    /// The dynamic properties of the tilemap.
    /// Any more than 65536 properties will not be saved.
    pub properties: HashMap<String, Property>,
}

/// A reason why reading a tilemap failed.
pub enum ReadError {
    /// IO error.
    IoError(io::Error),
    /// Invalid magic string.
    InvalidMagic,
    /// Unsupported version.
    UnsupportedVersion(u16),
    /// Invalid type in property map.
    InvalidType(u8),
    /// Layer length was not a multiple of two.
    InvalidLayerLength,
    /// Invalid header.
    InvalidHeader(String),
}
impl From<io::Error> for ReadError {
    fn from(err: io::Error) -> Self {
        ReadError::IoError(err)
    }
}

impl std::fmt::Debug for ReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::IoError(err) => write!(f, "{err}"),
            ReadError::UnsupportedVersion(v) => {
                write!(f, "version {v} of tilemap files is not supported")
            }
            ReadError::InvalidType(ty) => {
                write!(f, "found invalid type 0x{ty:02X} in property mapping")
            }
            ReadError::InvalidHeader(head) => write!(f, "found invalid header \"{head}\""),
            ReadError::InvalidMagic => write!(f, "found invalid magic string for tilemap"),
            ReadError::InvalidLayerLength => write!(f, "layer byte length did not match its size"),
        }
    }
}

impl Display for ReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ReadError {}

/// A helper struct to make writing headers easier.
struct Header<'a, 'b, W: Write> {
    stream: &'a mut W,
    buffer: Cursor<Vec<u8>>,
    header: &'b [u8],
}

impl<'a, 'b, W: Write> Header<'a, 'b, W> {
    #[must_use = "header won't write if dropped"]
    fn new(stream: &'a mut W, header: &'b [u8]) -> Self {
        Header {
            stream,
            buffer: Cursor::new(Vec::new()),
            header,
        }
    }

    fn write_header(self) -> io::Result<()> {
        self.stream.write_all(self.header)?;
        self.stream
            .write_all(&(self.buffer.get_ref().len() as u32).to_le_bytes())?;
        self.stream.write_all(self.buffer.get_ref())
    }
}

impl<'a, 'b, W: Write> Write for Header<'a, 'b, W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.buffer.write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

impl TileMap {
    /// Attempt to read a tilemap from a readable.
    ///
    /// # Errors
    /// Errors if the file fails to be read.
    pub fn read(mut cursor: impl Read) -> Result<Self, ReadError> {
        // Read the magic string, see if it matches
        let mut buf = [0; 8];
        cursor.read_exact(&mut buf)?;
        if &buf != b"ACHTUNG!" {
            return Err(ReadError::InvalidMagic);
        }
        // There's an extra bit flipped on for whatever reason
        // We get rid of it here
        let version = cursor.read_u16::<LittleEndian>()? ^ 0b1_0000_0000;
        if version > 5 {
            return Err(ReadError::UnsupportedVersion(version));
        }
        let mut tilemap = TileMap::default();
        let mut global_dimensions = (16, 16);
        loop {
            let mut block_id = [0; 4];
            if let Err(err) = cursor.read_exact(&mut block_id) {
                if matches!(err.kind(), io::ErrorKind::UnexpectedEof) {
                    // Reached EOF, stop
                    break;
                }
                // Other IO error, raise it
                return Err(ReadError::IoError(err));
            }
            // Block size is of no use to us
            let _block_size = cursor.read_u32::<LittleEndian>()?;
            match &block_id {
                b"MAP " => {
                    // Mapping of strings to arbitrary data
                    if version >= 3 {
                        let count = cursor.read_u16::<LittleEndian>()?;
                        for _ in 0..count {
                            let name = read_helper::read_short_string(&mut cursor)?;
                            let ty = cursor.read_u8()?;
                            let property = match ty {
                                // Integer
                                0 => Property::Integer(cursor.read_i32::<LittleEndian>()?),
                                1 => Property::Float(cursor.read_f32::<LittleEndian>()?),
                                2 => Property::String(read_helper::read_long_string(&mut cursor)?),
                                t => return Err(ReadError::InvalidType(t)),
                            };
                            let _ = tilemap
                                .properties
                                .insert(String::from_utf8_lossy(&name).into_owned(), property);
                        }
                    } else {
                        // Deprecated, only in older versions
                        global_dimensions = (
                            cursor.read_u16::<LittleEndian>()?,
                            cursor.read_u16::<LittleEndian>()?,
                        );
                    }
                }
                b"TILE" => {
                    let amount = cursor.read_u8()?;
                    for _ in 0..amount {
                        // Color is stored in xBGR
                        let mut buf = [0; 4];
                        cursor.read_exact(&mut buf)?;
                        let raw_path = read_helper::read_short_string(&mut cursor)?;
                        tilemap.tilesets.push(TileSet {
                            path: String::from_utf8_lossy(&raw_path).into_owned(),
                            transparent_color: (buf[3], buf[2], buf[1]),
                        });
                    }
                }
                b"LAYR" => {
                    let amount = if version == 0 {
                        cursor.read_u8()? as u16
                    } else {
                        cursor.read_u16::<LittleEndian>()?
                    };
                    for _ in 0..amount {
                        let mut layer = Layer::default();
                        let (width, height) = (
                            cursor.read_u32::<LittleEndian>()?,
                            cursor.read_u32::<LittleEndian>()?,
                        );
                        layer.width = width;
                        layer.height = height;
                        layer.tile_dimensions = if version >= 2 {
                            (
                                cursor.read_u16::<LittleEndian>()?,
                                cursor.read_u16::<LittleEndian>()?,
                            )
                        } else {
                            // Read global dimensions
                            global_dimensions
                        };
                        // Python struct syntax: =2B2i2f3?f
                        (
                            layer.tileset,
                            layer.collision,
                            layer.offset,
                            layer.scroll,
                            layer.wrap,
                            layer.visible,
                            layer.opacity,
                        ) = (
                            cursor.read_u8()?,
                            cursor.read_u8()?,
                            (
                                cursor.read_i32::<LittleEndian>()?,
                                cursor.read_i32::<LittleEndian>()?,
                            ),
                            (
                                cursor.read_f32::<LittleEndian>()?,
                                cursor.read_f32::<LittleEndian>()?,
                            ),
                            (cursor.read_u8()? > 0, cursor.read_u8()? > 0),
                            cursor.read_u8()? > 0,
                            cursor.read_f32::<LittleEndian>()?,
                        );
                        // Read sublayer link
                        if version >= 4 {
                            layer.sublayer_link.tileset = cursor.read_u8()?;
                            layer.sublayer_link.animation = cursor.read_u8()?;
                            if version == 5 {
                                layer.sublayer_link.animation_frame = cursor.read_u8()?;
                            }
                        }
                        // Read data blocks
                        let data_count = cursor.read_u8()?;
                        let mut header_buf = [0; 4];
                        for _ in 0..data_count {
                            cursor.read_exact(&mut header_buf)?;
                            match &header_buf {
                                b"MAIN" => {
                                    // Read the tiles
                                    let raw_tiles = read_helper::read_compressed(&mut cursor)?;
                                    if raw_tiles.len() % 2 != 0 {
                                        return Err(ReadError::InvalidLayerLength);
                                    }
                                    // We cannot do reinterpretation here,
                                    // since Tile.id has an alignment of 2,
                                    // while the vector has an alignment of 1.
                                    layer.data = raw_tiles
                                        .into_boxed_slice()
                                        .chunks(2)
                                        .map(|chunk| Tile {
                                            position: if cfg!(target_endian = "big") {
                                                [chunk[0], chunk[1]]
                                            } else {
                                                [chunk[1], chunk[0]]
                                            },
                                        })
                                        .collect();
                                }
                                b"DATA" => {
                                    let cell_size = cursor.read_u8()?.min(4);
                                    let mut default_value = [0; 4];
                                    cursor.read_exact(&mut default_value)?;
                                    let (w, h) = (layer.width, layer.height);
                                    let sublayer =
                                        layer.add_sublayer(&default_value[..cell_size as usize]);
                                    sublayer.resize(w, h);
                                    let sublayer_data = read_helper::read_compressed(&mut cursor)?;
                                    if sublayer_data.len()
                                        != (sublayer.width as usize
                                            * sublayer.height as usize
                                            * sublayer.cell_size as usize)
                                    {
                                        return Err(ReadError::InvalidLayerLength);
                                    }
                                    sublayer.data = sublayer_data;
                                }
                                header => {
                                    let header = String::from_utf8_lossy(header).into_owned();
                                    return Err(ReadError::InvalidHeader(header));
                                }
                            }
                        }
                        tilemap.layers.push(layer);
                    }
                }
                header => {
                    let header = String::from_utf8_lossy(header).into_owned();
                    return Err(ReadError::InvalidHeader(header));
                }
            }
        }
        Ok(tilemap)
    }

    /// Attempts to write a tilemap to a writable.
    ///
    /// # Errors
    /// The file failed to be written.
    pub fn write(&self, mut cursor: impl Write) -> Result<(), io::Error> {
        // Write magic string
        cursor.write_all(b"ACHTUNG!")?;
        // Always write version 5
        // The version has an extra bit
        cursor.write_u8(5)?;
        cursor.write_u8(1)?;
        if !self.properties.is_empty() {
            let mut cur = Header::new(&mut cursor, b"MAP ");
            // Can only store up to 65535 properties
            cur.write_u16::<LittleEndian>(self.properties.len().min(u16::MAX as usize) as u16)?;
            for (key, value) in self.properties.iter().take(0xFFFF) {
                write_helper::write_short_string(&mut cur, key)?;
                match value {
                    Property::Integer(i) => {
                        cur.write_u8(0)?; // Integer: 0
                        cur.write_i32::<LittleEndian>(*i)?;
                    }
                    Property::Float(f) => {
                        cur.write_u8(1)?; // Float: 1
                        cur.write_f32::<LittleEndian>(*f)?;
                    }
                    Property::String(s) => {
                        cur.write_u8(2)?; // String: 2
                        write_helper::write_long_string(&mut cur, s)?;
                    }
                }
            }
            cur.write_header()?;
        }
        if !self.tilesets.is_empty() {
            let mut cur = Header::new(&mut cursor, b"TILE");
            let len = self.tilesets.len().min(255) as u8;
            cur.write_u8(len)?;
            for tileset in self.tilesets.iter().take(0xFF) {
                cur.write_u8(0)?; // Padding
                cur.write_u8(tileset.transparent_color.2)?; // B
                cur.write_u8(tileset.transparent_color.1)?; // G
                cur.write_u8(tileset.transparent_color.0)?; // R
                write_helper::write_short_string(&mut cur, &tileset.path)?;
            }
            cur.write_header()?;
        }
        if !self.layers.is_empty() {
            let mut cur = Header::new(&mut cursor, b"LAYR");
            // Can only store up to 65535 layers
            cur.write_u16::<LittleEndian>(self.layers.len().min(u16::MAX as usize) as u16)?;
            for layer in self.layers.iter().take(0xFFFF) {
                cur.write_u32::<LittleEndian>(layer.width)?;
                cur.write_u32::<LittleEndian>(layer.height)?;
                // Write layer settings
                cur.write_u16::<LittleEndian>(layer.tile_dimensions.0)?;
                cur.write_u16::<LittleEndian>(layer.tile_dimensions.1)?;
                cur.write_u8(layer.tileset)?;
                cur.write_u8(layer.collision)?;
                cur.write_i32::<LittleEndian>(layer.offset.0)?;
                cur.write_i32::<LittleEndian>(layer.offset.1)?;
                cur.write_f32::<LittleEndian>(layer.scroll.0)?;
                cur.write_f32::<LittleEndian>(layer.scroll.1)?;
                cur.write_u8(layer.wrap.0 as u8)?;
                cur.write_u8(layer.wrap.1 as u8)?;
                cur.write_u8(layer.visible as u8)?;
                cur.write_f32::<LittleEndian>(layer.opacity)?;
                // Write sublayer link
                cur.write_u8(layer.sublayer_link.tileset)?;
                cur.write_u8(layer.sublayer_link.animation)?;
                cur.write_u8(layer.sublayer_link.animation_frame)?;
                if layer.width.min(layer.height) == 0 {
                    // Empty layer
                    cur.write_u8(0)?; // Layer size
                    continue;
                }
                // Number of headers in this section
                // Add one for the main header
                cur.write_u8((layer.sublayers.len() + 1).min(255) as u8)?;
                cur.write_all(b"MAIN")?;
                // Use bytemuck to safely cast the tiles
                let raw_tiles = layer.data.as_slice();
                let byte_slice: &[u8] = cast_slice(raw_tiles);
                write_helper::write_compressed(&mut cur, byte_slice)?;
                for sublayer in layer.sublayers.iter().take(255) {
                    cur.write_all(b"DATA")?;
                    cur.write_u8(sublayer.cell_size)?;
                    cur.write_all(&sublayer.default_value)?;
                    write_helper::write_compressed(&mut cur, sublayer.data.as_slice())?;
                }
            }
            cur.write_header()?;
        }
        Ok(())
    }

    /// Constructs a new instance from the default.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// A single layer of a tilemap.
#[derive(Clone, PartialEq)]
pub struct Layer {
    pub(crate) data: Vec<Tile>,
    /// Width of this layer.
    pub(crate) width: u32,
    /// Height of this layer.
    pub(crate) height: u32,
    /// Index of the tileset of this layer.
    pub tileset: u8,
    /// Index of the collision of this layer.
    pub collision: u8,
    /// The XY position offset of this layer.
    pub offset: (i32, i32),
    /// The XY scroll of this layer.
    pub scroll: (f32, f32),
    /// Which axes among XY this layer wraps on.
    pub wrap: (bool, bool),
    /// Whether the layer is visible.
    pub visible: bool,
    /// Opacity of this layer.
    pub opacity: f32,
    /// Dimensions of the tiles in this layer.
    pub tile_dimensions: (u16, u16),
    /// The sublayers of this layer.
    /// Any more than 255 sublayers will not be saved.
    pub sublayers: Vec<SubLayer>,
    /// The sublayer link of this layer.
    pub sublayer_link: SubLayerLink,
}

impl IntoIterator for Layer {
    type Item = Tile;
    type IntoIter = std::vec::IntoIter<Tile>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl Default for Layer {
    fn default() -> Self {
        Layer {
            data: Vec::new(),
            width: 0,
            height: 0,
            tileset: 0,
            collision: 0,
            offset: (0, 0),
            scroll: (0.0, 0.0),
            wrap: (false, false),
            visible: true,
            opacity: 1.0,
            tile_dimensions: (16, 16),
            sublayer_link: SubLayerLink::default(),
            sublayers: Vec::new(),
        }
    }
}

impl Layer {
    /// Resize the layer, filling empty tiles with the tile default (`0xFFFF`).
    ///
    /// If the width is changed, this will reallocate the data buffer!
    pub fn resize(&mut self, width: u32, height: u32) {
        if (self.width == width && self.height == height)
            || ((self.width == 0 || self.height == 0) && (width == 0 || height == 0))
        {
            // This does nothing!
            return;
        }
        if width == 0 || height == 0 {
            // Clear
            self.width = 0;
            self.height = 0;
            self.data.clear();
            for sublayer in &mut self.sublayers {
                sublayer.resize(width, height);
            }
            return;
        }
        if self.width == 0 || self.height == 0 {
            // Construct
            self.width = width;
            self.height = height;
            self.data = iter::repeat(Tile::default())
                .take((width * height) as usize)
                .collect();
            for sublayer in &mut self.sublayers {
                sublayer.resize(width, height);
            }
            return;
        }
        if self.height > height {
            // Remove rows
            self.data.truncate((self.width * height) as usize);
        } else if self.height < height {
            // Add rows
            self.data.extend(
                iter::repeat(Tile::default()).take((self.width * (height - self.height)) as usize),
            );
        }
        if self.width != width {
            let chunks = self.data.chunks(self.width as usize);
            self.data = if self.width < width {
                // Old less than new, add elements
                chunks
                    .flat_map(|chunk| {
                        chunk.iter().copied().chain(
                            iter::repeat(Tile::default()).take((width - self.width) as usize),
                        )
                    })
                    .collect()
            } else {
                // Truncate elements
                chunks
                    .flat_map(|chunk| chunk.iter().copied().take(width as usize))
                    .collect()
            };
        }
        self.width = width;
        self.height = height;
        for sublayer in &mut self.sublayers {
            sublayer.resize(width, height);
        }
    }

    /// Add a new sublayer to the layer, returning a mutable reference to it.
    pub fn add_sublayer(&mut self, default_value: &[u8]) -> &mut SubLayer {
        let mut sublayer = SubLayer::default();
        sublayer.set_default(default_value);
        sublayer.resize(self.width, self.height);
        self.sublayers.push(sublayer);
        // SAFETY: we literally just pushed to this
        unsafe { self.sublayers.last_mut().unwrap_unchecked() }
    }

    /// Returns the width of the layer.
    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of the layer.
    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get a tile by position.
    /// Returns None if out of bounds
    #[must_use]
    pub fn get(&self, (x, y): (usize, usize)) -> Option<&Tile> {
        let index = y * self.width as usize + x;
        self.data.get(index)
    }

    /// Get a tile by position, mutably.
    /// Returns None if out of bounds
    pub fn get_mut(&mut self, (x, y): (usize, usize)) -> Option<&mut Tile> {
        let index = y * self.width as usize + x;
        self.data.get_mut(index)
    }

    /// Constructs a new instance from the default.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Index<(usize, usize)> for Layer {
    type Output = Tile;

    /// Index by position and return a reference.
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    fn index(&self, (x, y): (usize, usize)) -> &Self::Output {
        let index = y * self.width as usize + x;
        &self.data[index]
    }
}

impl IndexMut<(usize, usize)> for Layer {
    /// Index by position and return a mutable reference.
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        let index = y * self.width as usize + x;
        &mut self.data[index]
    }
}

/// A tileset in the image.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct TileSet {
    /// Path to the tileset image.
    pub path: String,
    /// Color treated as transparent.
    pub transparent_color: (u8, u8, u8),
}

impl TileSet {
    /// Constructs a new instance from the default.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, PartialEq)]
/// A value of a property in a layer of a tilemap.
pub enum Property {
    /// Integer.
    Integer(i32),
    /// Floating point.
    Float(f32),
    /// Arbitrary bytes.
    /// Trying to write a string with length 0 to a file will fail!
    String(Vec<u8>),
}

impl From<i32> for Property {
    fn from(value: i32) -> Self {
        Self::Integer(value)
    }
}

impl From<f32> for Property {
    fn from(value: f32) -> Self {
        Self::Float(value)
    }
}

impl From<Vec<u8>> for Property {
    fn from(value: Vec<u8>) -> Self {
        Self::String(value)
    }
}

impl From<String> for Property {
    fn from(value: String) -> Self {
        Self::String(value.into_bytes())
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
/// A union representing a tile in a tilemap.
///
/// The ID should always be stored using big endian.
/// If it is not, X and Y will be swapped, which is undesirable.
/// Be careful!
pub union Tile {
    /// Identifier
    pub id: u16,
    /// XY position
    pub position: [u8; 2],
}

// SAFETY: These both hold for both fields.
unsafe impl Zeroable for Tile {}
unsafe impl Pod for Tile {}

impl Tile {
    /// Safely returns the tile's ID.
    // SAFETY: The size and alignment of u16
    // is the same as the struct.
    #[must_use]
    pub fn id(&self) -> u16 {
        unsafe { self.id }
    }
    /// Safely returns a mutable reference to the tile's ID.
    ///
    /// Make sure any writes to this are big endian!
    /// Little endian won't be unsound, but will swap X and Y
    /// when reading position.
    // SAFETY: See above.
    #[must_use]
    pub fn id_mut(&mut self) -> &mut u16 {
        unsafe { &mut self.id }
    }
    /// Safely returns a reference to the tile's position.
    // SAFETY: The size of [u8; 2]
    // is the same as u16, and [u8; 2]
    // is not #repr(Rust).
    #[must_use]
    pub fn position(&self) -> [u8; 2] {
        unsafe { self.position }
    }
    /// Safely returns a mutable reference to the tile's position.
    // SAFETY: See above.
    #[must_use]
    pub fn position_mut(&mut self) -> &mut [u8; 2] {
        unsafe { &mut self.position }
    }
}

impl Default for Tile {
    fn default() -> Self {
        Self { id: 0xFFFF }
    }
}

impl PartialEq for Tile {
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: All fields are of the same type,
        // and all bit patterns are valid for said fields.
        unsafe { self.id == other.id }
    }
}

/// A sublayer within a layer of a tilemap.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct SubLayer {
    pub(crate) data: Vec<u8>,
    default_value: [u8; 4],
    cell_size: u8,
    width: u32,
    height: u32,
}

impl SubLayer {
    /// Resize the sublayer, filling empty tiles with the sublayer's default value.
    ///
    /// If the width is changed, this will reallocate the data buffer!
    ///
    /// # Sanity
    /// The layer this is put into should be the same size as the new size.
    ///
    /// # Panics
    /// Panics if the resulting area overflows a u32.
    pub fn resize(&mut self, width: u32, height: u32) {
        if (self.width == width && self.height == height)
            || ((self.width == 0 || self.height == 0) && (width == 0 || height == 0))
        {
            // This does nothing!
            return;
        }
        if width == 0 || height == 0 {
            // Clear
            self.width = 0;
            self.height = 0;
            self.data.clear();
            return;
        }
        let default = &self.default_value[..self.cell_size as usize];
        if self.width == 0 || self.height == 0 {
            // Construct
            self.width = width;
            self.height = height;
            self.data = iter::repeat(default)
                .take((width * height) as usize)
                .flatten()
                .copied()
                .collect();
            return;
        }
        if self.height > height {
            // Remove rows
            self.data
                .truncate((self.width * height * self.cell_size as u32) as usize);
        } else if self.height < height {
            // Add rows
            self.data.extend(
                iter::repeat(default)
                    .take((self.width * (height - self.height)) as usize)
                    .flatten(),
            );
        }
        if self.width != width {
            let chunks = self
                .data
                .chunks(self.width as usize * self.cell_size as usize);
            self.data = if self.width < width {
                // Old less than new, add elements
                chunks
                    .flat_map(|chunk| {
                        chunk.iter().copied().chain(
                            iter::repeat(default)
                                .take((width - self.width) as usize)
                                .flatten()
                                .copied(),
                        )
                    })
                    .collect()
            } else {
                // Truncate elements
                chunks
                    .flat_map(|chunk| chunk.iter().take(self.cell_size as usize * width as usize))
                    .copied()
                    .collect()
            };
        }
        self.width = width;
        self.height = height;
    }

    /// Returns the size of one data cell.
    #[inline]
    #[must_use]
    pub fn cell_size(&self) -> u8 {
        self.cell_size
    }

    /// Returns the width of the sublayer.
    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of the sublayer.
    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Set the default value of the sublayer, resizing all cells to its length.
    ///
    /// The default value is truncated to 4 bytes if larger.
    ///
    /// If the new default value is larger than the old one, all cells are zero padded to the new length.
    ///
    /// If the new default is smaller, all cells are truncated to its length.
    ///
    /// This will *only* not reallocate if the length of the new default is the same as the old one!
    pub fn set_default(&mut self, default: &[u8]) {
        let old_size = self.cell_size as usize;
        let new_size = default.len().min(4);
        let default = default.to_vec();
        let mut spaced_default = default.clone();
        spaced_default.resize(4, 0);
        // SAFETY: due to resizing, this is always exactly 4 bytes long
        let spaced_default_slice: &[u8; 4] =
            unsafe { spaced_default.as_slice().try_into().unwrap_unchecked() };
        self.default_value = *spaced_default_slice;
        self.cell_size = new_size as u8;
        if new_size == old_size || self.width == 0 || self.height == 0 {
            // No need to resize the cells
            return;
        }
        if new_size == 0 {
            // Cell size is zero
            self.data = Vec::new();
            return;
        }
        if old_size == 0 {
            // Need to construct
            self.data.resize(
                (self.width * self.height * self.cell_size as u32) as usize,
                0,
            );
            return;
        }
        // Resize each cell of the sublayer to the value's size
        self.data = if new_size > old_size {
            self.data
                .as_slice()
                .chunks(old_size)
                .flat_map(|cell| {
                    // Need to 0-pad
                    cell.iter()
                        .chain(iter::repeat(&0).take(new_size - old_size))
                })
                .copied()
                .collect()
        } else {
            self.data
                .as_slice()
                .chunks(old_size)
                .flat_map(|cell| {
                    // Need to 0-pad
                    cell.iter().take(new_size)
                })
                .copied()
                .collect()
        };
    }

    /// Get a cell by position.
    /// Returns None if out of bounds.
    #[must_use]
    pub fn get(&self, (x, y): (u32, u32)) -> Option<&[u8]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let size = self.cell_size as usize;
        let start = (y * self.width + x) as usize * size;
        let end = start + size;
        Some(&self.data[start..end])
    }

    /// Get a cell by position, mutably.
    /// Returns None if out of bounds
    pub fn get_mut(&mut self, (x, y): (u32, u32)) -> Option<&mut [u8]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let size = self.cell_size as usize;
        let start = (y * self.width + x) as usize * size;
        let end = start + size;
        Some(&mut self.data[start..end])
    }

    /// Constructs a new instance from the default.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Index<(u32, u32)> for SubLayer {
    type Output = [u8];

    /// Index by position and return a reference.
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    fn index(&self, (x, y): (u32, u32)) -> &Self::Output {
        let size = self.cell_size as usize;
        let start = (y * self.width + x) as usize * size;
        let end = start + size;
        &self.data[start..end]
    }
}

impl IndexMut<(u32, u32)> for SubLayer {
    /// Index by position and return a mutable reference.
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    fn index_mut(&mut self, (x, y): (u32, u32)) -> &mut Self::Output {
        let size = self.cell_size as usize;
        let start = (y * self.width + x) as usize * size;
        let end = start + size;
        &mut self.data[start..end]
    }
}

/// A link to a sublayer within a layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubLayerLink {
    /// Which sublayer is this layer's tileset linked to?
    pub tileset: u8,
    /// Which sublayer is this layer's animation linked to?
    pub animation: u8,
    /// Which sublayer is this layer's animation frames linked to?
    pub animation_frame: u8,
}

impl SubLayerLink {
    /// Constructs a new instance from the default.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for SubLayerLink {
    fn default() -> Self {
        Self {
            tileset: 0xFF,
            animation: 0xFF,
            animation_frame: 0xFF,
        }
    }
}
