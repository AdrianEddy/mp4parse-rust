// Module for parsing ISO Base Media Format aka video/mp4 streams.

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/// Basic ISO box structure.
pub struct BoxHeader {
    /// Four character box type
pub name: u32,
    /// Size of the box in bytes
pub size: u64,
    /// Offset to the start of the contained data (or header size).
pub offset: u64,
}

/// File type box 'ftyp'.
pub struct FileTypeBox {
    name: u32,
    size: u64,
    major_brand: u32,
    minor_version: u32,
    compatible_brands: Vec<u32>,
}

/// Movie header box 'mvhd'.
pub struct MovieHeaderBox {
    pub name: u32,
    pub size: u64,
    pub timescale: u32,
    pub duration: u64,
    // Ignore other fields.
}

extern crate byteorder;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Result, Seek, SeekFrom};

/// Parse a box out of a data buffer.
pub fn read_box_header<T: ReadBytesExt>(src: &mut T) -> Option<BoxHeader> {
    let tmp_size = src.read_u32::<BigEndian>().unwrap();
    let name = src.read_u32::<BigEndian>().unwrap();
    let size = match tmp_size {
        1 => src.read_u64::<BigEndian>().unwrap(),
        _ => tmp_size as u64,
    };
    assert!(size >= 8);
    if tmp_size == 1 {
        assert!(size >= 16);
    }
    let offset = match tmp_size {
        1 => 4 + 4 + 8,
        _ => 4 + 4,
    };
    assert!(offset <= size);
    Some(BoxHeader{
        name: name,
        size: size,
        offset: offset,
    })
}

/// Parse the extra header fields for a full box.
fn read_fullbox_extra<T: ReadBytesExt>(src: &mut T) -> (u8, u32) {
    let version = src.read_u8().unwrap();
    let flags_a = src.read_u8().unwrap();
    let flags_b = src.read_u8().unwrap();
    let flags_c = src.read_u8().unwrap();
    (version, (flags_a as u32) << 16 |
              (flags_b as u32) <<  8 |
              (flags_c as u32))
}

/// Skip over the contents of a box.
pub fn skip_box_content<T: ReadBytesExt + Seek>
  (src: &mut T, header: &BoxHeader)
  -> std::io::Result<u64>
{
    src.seek(SeekFrom::Current((header.size - header.offset) as i64))
}

/// Parse an ftype box.
pub fn read_ftyp<T: ReadBytesExt>(src: &mut T, head: &BoxHeader)
  -> Option<FileTypeBox> {
    let major = src.read_u32::<BigEndian>().unwrap();
    let minor = src.read_u32::<BigEndian>().unwrap();
    let brand_count = (head.size - 8 - 8) /4;
    let mut brands = Vec::new();
    for _ in 0..brand_count {
        brands.push(src.read_u32::<BigEndian>().unwrap());
    }
    Some(FileTypeBox{
        name: head.name,
        size: head.size,
        major_brand: major,
        minor_version: minor,
        compatible_brands: brands,
    })
}

/// Parse an mvhd box.
pub fn read_mvhd<T: ReadBytesExt>(src: &mut T, head: &BoxHeader)
  -> Option<MovieHeaderBox> {
    let (version, _) = read_fullbox_extra(src);
    match version {
        1 => {
            // 64 bit creation and modification times.
            let mut skip: Vec<u8> = vec![0; 16];
            let r = src.read(&mut skip).unwrap();
            assert!(r == skip.len());
        },
        0 => {
            // 32 bit creation and modification times.
            // 64 bit creation and modification times.
            let mut skip: Vec<u8> = vec![0; 8];
            let r = src.read(&mut skip).unwrap();
            assert!(r == skip.len());
        },
        _ => panic!("invalid mhdr version"),
    }
    let timescale = src.read_u32::<BigEndian>().unwrap();
    let duration = match version {
        1 => src.read_u64::<BigEndian>().unwrap(),
        0 => src.read_u32::<BigEndian>().unwrap() as u64,
        _ => panic!("invalid mhdr version"),
    };
    Some(MovieHeaderBox {
        name: head.name,
        size: head.size,
        timescale: timescale,
        duration: duration,
    })
}

/// Convert the iso box type or other 4-character value to a string.
pub fn fourcc_to_string(name: u32) -> String {
    let u32_to_vec = |u| {
        vec!((u >> 24 & 0xffu32) as u8,
             (u >> 16 & 0xffu32) as u8,
             (u >>  8 & 0xffu32) as u8,
             (u & 0xffu32) as u8)
    };
    let name_bytes = u32_to_vec(name);
    String::from_utf8_lossy(&name_bytes).into_owned()
}

use std::fmt;
impl fmt::Display for BoxHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "'{}' {} bytes", fourcc_to_string(self.name), self.size)
    }
}

impl fmt::Display for FileTypeBox {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = fourcc_to_string(self.name);
        let brand = fourcc_to_string(self.major_brand);
        write!(f, "'{}' {} bytes '{}' v{}", name, self.size,
            brand, self.minor_version)
    }
}

impl fmt::Display for MovieHeaderBox {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = fourcc_to_string(self.name);
        write!(f, "'{}' {} bytes duration {}s", name, self.size,
            (self.duration as f64)/(self.timescale as f64))
    }
}

#[test]
fn test_read_box_header() {
    use std::io::Cursor;
    use std::io::Write;
    let mut test: Vec<u8> = vec![0, 0, 0, 8];  // minimal box length
    write!(&mut test, "test").unwrap(); // box type
    let mut stream = Cursor::new(test);
    let parsed = read_box_header(&mut stream).unwrap();
    assert_eq!(parsed.name, 1952805748);
    assert_eq!(parsed.size, 8);
    println!("box {}", parsed);
}


#[test]
fn test_read_box_header_long() {
    use std::io::Cursor;
    let mut test: Vec<u8> = vec![0, 0, 0, 1]; // long box extension code
    test.extend("long".to_string().into_bytes()); // box type
    test.extend(vec![0, 0, 0, 0, 0, 0, 16, 0]); // 64 bit size
    // Skip generating box content.
    let mut stream = Cursor::new(test);
    let parsed = read_box_header(&mut stream).unwrap();
    assert_eq!(parsed.name, 1819242087);
    assert_eq!(parsed.size, 4096);
    println!("box {}", parsed);
}

#[test]
fn test_read_ftyp() {
    use std::io::Cursor;
    use std::io::Write;
    let mut test: Vec<u8> = vec![0, 0, 0, 24]; // size
    write!(&mut test, "ftyp").unwrap(); // type
    write!(&mut test, "mp42").unwrap(); // major brand
    test.extend(vec![0, 0, 0, 0]);      // minor version
    write!(&mut test, "isom").unwrap(); // compatible brands...
    write!(&mut test, "mp42").unwrap();
    assert_eq!(test.len(), 24);

    let mut stream = Cursor::new(test);
    let header = read_box_header(&mut stream).unwrap();
    let parsed = read_ftyp(&mut stream, &header).unwrap();
    assert_eq!(parsed.name, 1718909296);
    assert_eq!(parsed.size, 24);
    assert_eq!(parsed.major_brand, 1836069938);
    assert_eq!(parsed.minor_version, 0);
    assert_eq!(parsed.compatible_brands.len(), 2);
    println!("box {}", parsed);
}
