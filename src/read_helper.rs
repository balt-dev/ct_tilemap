use byteorder::{LittleEndian, ReadBytesExt};
use libflate::zlib::Decoder;
use std::io;
use std::io::{Cursor, Read};

pub(crate) fn read_short_string(mut r: impl Read) -> io::Result<Vec<u8>> {
    let length = r.read_u8()? as usize + 1;
    let mut bytes = vec![0u8; length];
    // Read that many bytes into the vector
    r.read_exact(bytes.as_mut_slice())?;
    Ok(bytes)
}

pub(crate) fn read_long_string(mut r: impl Read) -> io::Result<Vec<u8>> {
    let length = r.read_u32::<LittleEndian>()? as usize + 1;
    if length > isize::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "string size was too large to fit in a byte vector",
        ));
    }
    // Allocate enough length ahead of time
    let mut bytes = vec![0u8; length];
    // Read that many bytes into the vector
    r.read_exact(bytes.as_mut_slice())?;
    Ok(bytes)
}

pub(crate) fn read_compressed(mut r: impl Read) -> io::Result<Vec<u8>> {
    let length = r.read_u32::<LittleEndian>()? as usize;
    if length > isize::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "compressed data size was too large to fit in a byte vector",
        ));
    }
    // Decode data using libflate
    let mut encoded_buf = vec![0; length];
    let encoded = encoded_buf.as_mut_slice();
    r.read_exact(encoded)?;
    let mut decoder = Decoder::new(Cursor::new(encoded))?;
    let mut decoded_buf = Vec::new();
    decoder.read_to_end(&mut decoded_buf)?;
    Ok(decoded_buf)
}
