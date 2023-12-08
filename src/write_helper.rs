use byteorder::{LittleEndian, WriteBytesExt};
use libflate::zlib::Encoder;
use std::io;
use std::io::{Cursor, Write};

pub(crate) fn write_short_string(mut w: impl Write, string: &str) -> io::Result<()> {
    let mut bytes = string.as_bytes();
    let mut len = bytes.len().min(256);
    if len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cannot write an empty string",
        ));
    }
    len -= 1;
    w.write_u8(len as u8)?;
    if bytes.len() > 256 {
        bytes = &bytes[..256];
    }
    w.write_all(bytes)
}

pub(crate) fn write_long_string(mut w: impl Write, string: &[u8]) -> io::Result<()> {
    let len = string.len();
    if len > u32::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "string size was too large to fit in the file",
        ));
    } else if len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cannot write an empty string",
        ));
    }
    w.write_u32::<LittleEndian>((len - 1) as u32)?;
    w.write_all(string)
}

pub(crate) fn write_compressed(mut w: impl Write, data: &[u8]) -> io::Result<()> {
    let mut buf = Cursor::new(Vec::new());
    let mut encoder = Encoder::new(&mut buf)?;
    encoder.write_all(data)?;
    encoder.finish().into_result()?;
    w.write_u32::<LittleEndian>(buf.position() as u32)?;
    w.write_all(buf.get_ref().as_slice())?;
    Ok(())
}
