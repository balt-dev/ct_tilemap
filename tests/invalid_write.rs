use std::io::{Cursor, Write};
use ct_tilemap::TileMap;


struct TrashWriter;
impl Write for TrashWriter {
   fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {Ok(buf.len())}
   fn flush(&mut self) -> std::io::Result<()> {Ok(())}
}

#[test]
fn invalid_writes() {
    let _ = TrashWriter.flush(); // Here to satisfy coverage checker
    let t = TileMap::default();
    // Test empty strings
    let mut str_test = t.clone();
    str_test.properties.insert(String::new(), 0.into());
    str_test.write(TrashWriter).expect_err("should have failed to write empty string");
    let mut str_test = t.clone();
    str_test.properties.insert("Foo".into(), String::new().into());
    str_test.write(TrashWriter).expect_err("should have failed to write empty string");
    let mut str_test = t.clone();
    str_test.properties.insert(
        "This is a very long string! I know this because I wrote it. This string has to be at least 256 characters long to trigger the error, so I'm intentionally dragging this on as long as I can to reach that limit. It's kind of annoying to have to write all this.".into(),
        0.into()
    );

    // Test string truncation
    let mut cur = Cursor::new(Vec::new());
    str_test.write(&mut cur).expect("this doesn't fail");
    let t = TileMap::read(cur.get_ref().as_slice()).expect("reading should not fail here");
    assert_eq!(t.properties.keys().next().expect("key should exist").len(), 256);
}