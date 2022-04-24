use gml::symbol::Symbol;
use gml::{self, vm};

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Error {
    NoINI,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Error::NoINI => {
                write!(f, "can't read or write to INI when none is open")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

#[derive(Default)]
pub struct State {
    current_path: Option<PathBuf>,
}

trait FileLike: Read + Write + Seek {
    fn set_len(&mut self, size: u64) -> std::io::Result<()>;
}

impl FileLike for File {
    fn set_len(&mut self, size: u64) -> std::io::Result<()> {
        File::set_len(self, size)
    }
}

#[cfg(test)]
impl FileLike for std::io::Cursor<&mut Vec<u8>> {
    fn set_len(&mut self, size: u64) -> std::io::Result<()> {
        self.get_mut().resize(size as usize, 0);
        Ok(())
    }
}

enum Op<'a> {
    Read,
    WriteOrDelete(Option<&'a [u8]>),
}

// This is not the prettiest or fastest way to manipulate an INI file, but this
// is probably how the Windows API and thus Game Maker do it.
// Importantly, it is non-destructive.
fn read_or_write_key<F: FileLike>(
    file: F,
    section: &[u8],
    key: &[u8],
    op: Op<'_>,
) -> Option<Vec<u8>> {
    let mut buffered_file = BufReader::new(file);

    let mut found_section = false;
    let mut buf: Vec<u8> = Vec::new();

    let mut line_terminated = false;

    while buffered_file.read_until(b'\n', &mut buf).is_ok() && buf.len() > 0 {
        let line_end = buffered_file.stream_position().unwrap();
        let line_start = line_end - buf.len() as u64;

        // TODO: more whitespace handling?
        line_terminated = buf.ends_with(b"\r\n");
        if line_terminated {
            buf.truncate(buf.len() - 2)
        }

        if let Some((&b'[', rest)) = buf.split_first() {
            if let Some((&b']', current_section)) = rest.split_last() {
                let old_found_section = found_section;
                found_section = section == current_section;
                if let Op::WriteOrDelete(Some(new_value)) = op {
                    if !found_section && old_found_section {
                        // Didn't find the value in the section. Add it to the
                        // end of the section.

                        let mut file = buffered_file.into_inner();
                        file.seek(SeekFrom::Start(line_start)).unwrap();
                        buf.clear();
                        file.read_to_end(&mut buf).unwrap();
                        file.seek(SeekFrom::Start(line_start)).unwrap();
                        file.write_all(key).unwrap();
                        file.write_all(b"=").unwrap();
                        file.write_all(new_value).unwrap();
                        file.write_all(b"\r\n").unwrap();
                        file.write_all(&buf).unwrap();
                        return None;
                    }
                }
            }
        }

        if found_section {
            if let Some(&[b'=', ..]) = buf.strip_prefix(key) {
                if let Op::WriteOrDelete(new_value) = op {
                    // Replace or delete existing value.

                    let mut file = buffered_file.into_inner();
                    file.seek(SeekFrom::Start(line_end)).unwrap();

                    if let Some(new_value) = new_value {
                        buf.drain((key.len() + 1)..);
                        buf.extend_from_slice(new_value);
                        buf.extend_from_slice(b"\r\n");
                    } else {
                        buf.clear();
                    }
                    file.read_to_end(&mut buf).unwrap();
                    file.seek(SeekFrom::Start(line_start)).unwrap();
                    file.write_all(&buf).unwrap();
                    // Truncate file if the new value is shorter.
                    let new_len = file.stream_position().unwrap();
                    file.set_len(new_len).unwrap();
                    return None;
                } else {
                    buf.drain(0..(key.len() + 1));
                    return Some(buf);
                }
            }
        }

        buf.clear();
    }

    if let Op::WriteOrDelete(Some(new_value)) = op {
        // We didn't find the section, or the section was the last in the file.
        // Append new lines as appropriate.

        let mut file = buffered_file.into_inner();
        file.seek(SeekFrom::End(0)).unwrap();
        if !line_terminated {
            file.write_all(b"\r\n").unwrap();
        }
        if !found_section {
            file.write_all(b"[").unwrap();
            file.write_all(section).unwrap();
            file.write_all(b"]\r\n").unwrap();
        }
        file.write_all(key).unwrap();
        file.write_all(b"=").unwrap();
        file.write_all(new_value).unwrap();
        file.write_all(b"\r\n").unwrap();
    }

    None
}

#[gml::bind]
impl State {
    #[gml::api]
    pub fn ini_open(&mut self, filename: Symbol) {
        // Despite the names, ini_open() and ini_close() do not use a persistent
        // file handle. The file is only opened when read from or written to.
        // This comes from the underlying Windows API functions,
        // GetPrivateProfileString and WritePrivateProfileString, which take the
        // INI file name as a parameter.

        // TODO: GML expects the filename to be a relative path for the same
        // directory as the game, e.g. "game.ini". It rejects absolute paths.
        // TODO: Use appropriate encoding (e.g. Windows-1252)
        let path_buf = PathBuf::from(std::str::from_utf8(&filename).unwrap());
        self.current_path = Some(path_buf);
    }

    #[gml::api]
    pub fn ini_close(&mut self) {
        self.current_path = None;
    }

    fn get_path(&self) -> vm::Result<&Path> {
        Ok(self.current_path.as_deref().ok_or(Error::NoINI)?)
    }

    fn read_key(&mut self, section: &[u8], key: &[u8]) -> vm::Result<Option<Vec<u8>>> {
        let path = self.get_path()?;
        // We only want to create the file if we're writing to it. The file not
        // existing is never an error.
        if let Ok(file) = OpenOptions::new().read(true).open(path) {
            Ok(read_or_write_key(file, &section, &key, Op::Read))
        } else {
            Ok(None)
        }
    }

    fn write_or_delete_key(
        &mut self,
        section: &[u8],
        key: &[u8],
        value: Option<&[u8]>,
    ) -> vm::Result<()> {
        let path = self.get_path()?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        read_or_write_key(file, section, key, Op::WriteOrDelete(value));
        Ok(())
    }

    #[gml::api]
    pub fn ini_read_string(
        &mut self,
        section: Symbol,
        key: Symbol,
        default: vm::ValueRef,
    ) -> vm::Result<vm::Value> {
        if let Some(found_value) = self.read_key(&section, &key)? {
            Ok(Symbol::intern(&found_value).into())
        } else {
            // TODO: Default value might need to be converted. GM8 doesn't
            //       preserve the type of default values that don't match the
            //       type in the function name.
            Ok(default.clone())
        }
    }

    #[gml::api]
    pub fn ini_write_string(
        &mut self,
        section: Symbol,
        key: Symbol,
        value: Symbol,
    ) -> vm::Result<()> {
        self.write_or_delete_key(&section, &key, Some(&value))
    }

    #[gml::api]
    pub fn ini_read_real(
        &mut self,
        section: Symbol,
        key: Symbol,
        default: vm::ValueRef,
    ) -> vm::Result<vm::Value> {
        if let Some(found_value) = self.read_key(&section, &key)? {
            // same implementation as real(). have not verified GM does this.
            let str = std::str::from_utf8(&found_value).unwrap_or("");
            Ok(str.parse::<f64>().unwrap_or(0.0).into())
        } else {
            // Same type conversion TODO applies here
            Ok(default.clone())
        }
    }

    #[gml::api]
    pub fn ini_write_real(&mut self, section: Symbol, key: Symbol, value: f64) -> vm::Result<()> {
        let value = format!("{}", value);
        self.write_or_delete_key(&section, &key, Some(value.as_bytes()))
    }

    #[gml::api]
    pub fn ini_key_delete(&mut self, section: Symbol, key: Symbol) -> vm::Result<()> {
        self.write_or_delete_key(&section, &key, None)
    }

    // TODO: ini_key_exists(), ini_section_exists(), ini_section_delete()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn ini_read() {
        let mut file = Vec::new();
        file.extend_from_slice(b"[Sec1]\r\nFoo=Bar\r\n\r\n[Sec2]\r\nA=B\r\n");
        let cursor = Cursor::new(&mut file);
        assert_eq!(
            read_or_write_key(cursor, b"Sec1", b"Foo", Op::Read).unwrap(),
            b"Bar"
        );
        let cursor = Cursor::new(&mut file);
        assert_eq!(read_or_write_key(cursor, b"Sec1", b"A", Op::Read), None);
        let cursor = Cursor::new(&mut file);
        assert_eq!(
            read_or_write_key(cursor, b"Sec2", b"A", Op::Read).unwrap(),
            b"B"
        );
        let cursor = Cursor::new(&mut file);
        assert_eq!(read_or_write_key(cursor, b"Sec2", b"Foo", Op::Read), None);
    }

    fn test_write_or_delete(
        input: &[u8],
        section: &[u8],
        key: &[u8],
        value: Option<&[u8]>,
        expect: &[u8],
    ) {
        let mut file = Vec::new();
        file.extend_from_slice(input);
        let cursor = Cursor::new(&mut file);
        read_or_write_key(cursor, section, key, Op::WriteOrDelete(value));
        assert_eq!(&file, expect);
    }

    #[test]
    fn ini_write_new_key_end_of_section() {
        test_write_or_delete(
            b"[Section]\r\nKey=Val1\r\n[Section2]",
            b"Section",
            b"Key2",
            Some(b"Val2"),
            b"[Section]\r\nKey=Val1\r\nKey2=Val2\r\n[Section2]",
        );
    }

    #[test]
    fn ini_write_new_key_end_of_section_nocrlf() {
        test_write_or_delete(
            b"[Section]\r\nKey=Val1\r\n[Section2]\r\n",
            b"Section",
            b"Key2",
            Some(b"Val2"),
            b"[Section]\r\nKey=Val1\r\nKey2=Val2\r\n[Section2]\r\n",
        );
    }

    #[test]
    fn ini_write_new_key_end_of_file() {
        test_write_or_delete(
            b"[Section]\r\nKey=Val1\r\n",
            b"Section",
            b"Key2",
            Some(b"Val2"),
            b"[Section]\r\nKey=Val1\r\nKey2=Val2\r\n",
        );
    }

    #[test]
    fn ini_write_new_key_end_of_file_nocrlf() {
        test_write_or_delete(
            b"[Section]\r\nKey=Val1",
            b"Section",
            b"Key2",
            Some(b"Val2"),
            b"[Section]\r\nKey=Val1\r\nKey2=Val2\r\n",
        );
    }

    #[test]
    fn ini_write_new_section_end_of_file() {
        test_write_or_delete(
            b"[Section]\r\nKey=Val1\r\n",
            b"Section2",
            b"Key",
            Some(b"Val2"),
            b"[Section]\r\nKey=Val1\r\n[Section2]\r\nKey=Val2\r\n",
        );
    }

    #[test]
    fn ini_write_new_section_end_of_file_nocrlf() {
        test_write_or_delete(
            b"[Section]\r\nKey=Val1",
            b"Section2",
            b"Key",
            Some(b"Val2"),
            b"[Section]\r\nKey=Val1\r\n[Section2]\r\nKey=Val2\r\n",
        );
    }

    #[test]
    fn ini_write_new_value_shorter() {
        test_write_or_delete(
            b"[Section]\r\nKey=LooooooooooongValue\r\n[Section2]\r\n",
            b"Section",
            b"Key",
            Some(b"ShortVal"),
            b"[Section]\r\nKey=ShortVal\r\n[Section2]\r\n",
        );
    }

    #[test]
    fn ini_write_new_value_longer() {
        test_write_or_delete(
            b"[Section]\r\nKey=ShortVal\r\n[Section2]\r\n",
            b"Section",
            b"Key",
            Some(b"LooooooooooongValue"),
            b"[Section]\r\nKey=LooooooooooongValue\r\n[Section2]\r\n",
        );
    }

    #[test]
    fn ini_delete_key() {
        test_write_or_delete(
            b"[Section]\r\nKey=LooooooooooongValue\r\n[Section2]\r\n",
            b"Section",
            b"Key",
            None,
            b"[Section]\r\n[Section2]\r\n",
        );
    }
}
