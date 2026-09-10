// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Reading a picture's size and kind from the first few bytes of it.
//!
//! Needed to decide whether a cover found on another platform is genuinely
//! better than the one already saved, and to give the saved file the right
//! name (#1159).
//!
//! Two mistakes this exists to prevent, both found in review:
//!
//! * **Replacing a good cover with a worse one.** Without measuring what is
//!   already there, any candidate wins, so a lower-resolution picture could
//!   overwrite a higher-resolution one the person already had.
//! * **Naming a file after something it is not.** The setting offers "raw" as
//!   a format, which is not a picture format at all — it means "whatever the
//!   source gives us". Using that name produced `Cover.raw`, which no picture
//!   viewer opens, sitting alongside the real cover rather than replacing it.
//!   Writing JPEG data into a file called `.png` has the same shape of
//!   problem.
//!
//! Only the two formats album art actually arrives in are read. Anything else
//! is reported as unknown, and the caller then leaves well alone rather than
//! guessing.

/// What a picture is, and how big.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The file extension this picture should be saved under.
    pub extension: &'static str,
}

impl ImageInfo {
    /// How many pixels the picture has in total.
    ///
    /// The honest way to compare two pictures. File size is not: a bigger file
    /// is often the same picture saved less efficiently. Neither is width or
    /// height alone: a very wide, short picture can have fewer pixels than a
    /// smaller square one.
    #[must_use]
    pub fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

/// Reads a picture's size and kind from the start of its data.
///
/// # Arguments
///
/// * `bytes` -- The picture's data. Only the first few dozen bytes are read
///   for a PNG; a JPEG may need more, since its size is recorded partway in.
///
/// # Returns
///
/// The size and file extension, or `None` when the data is not a picture this
/// understands or is too short to tell.
#[must_use]
pub fn read_image_info(bytes: &[u8]) -> Option<ImageInfo> {
    read_png(bytes).or_else(|| read_jpeg(bytes))
}

/// Reads a PNG's size.
///
/// PNG puts its size in a fixed place: the eight-byte signature, then a
/// chunk header, then width and height as four bytes each. So it is a simple
/// read at a known offset.
fn read_png(bytes: &[u8]) -> Option<ImageInfo> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 24 || bytes[..8] != SIGNATURE {
        return None;
    }
    // Bytes 12..16 name the chunk; the first must be the header chunk.
    if &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    Some(ImageInfo {
        width,
        height,
        extension: "png",
    })
}

/// Reads a JPEG's size.
///
/// JPEG is a series of segments, and the size lives in whichever segment
/// describes the frame. That is not at a fixed position, so the segments have
/// to be walked until one of those is found.
fn read_jpeg(bytes: &[u8]) -> Option<ImageInfo> {
    // Every JPEG starts with the "start of image" marker.
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }

    let mut i = 2usize;
    // Bounded by the data itself; each step moves forward, so this ends.
    while i + 3 < bytes.len() {
        // Segments begin with 0xFF. Padding bytes of 0xFF may appear between
        // them and are skipped.
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        // Markers that carry no length and cannot hold a size.
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        // The frame-describing markers. Height and width sit three and five
        // bytes into the segment's contents. The four excluded values in this
        // range are not frame markers.
        let describes_the_frame = (0xC0..=0xCF).contains(&marker)
            && marker != 0xC4
            && marker != 0xC8
            && marker != 0xCC;
        let length = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if describes_the_frame {
            if i + 9 >= bytes.len() {
                return None;
            }
            let height = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]);
            let width = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]);
            if width == 0 || height == 0 {
                return None;
            }
            return Some(ImageInfo {
                width: u32::from(width),
                height: u32::from(height),
                extension: "jpg",
            });
        }
        // A length below two would not move forward, and would spin here.
        if length < 2 {
            return None;
        }
        i += 2 + length;
    }
    None
}

/// Reads a picture's size and kind from a file, if it can.
///
/// # Arguments
///
/// * `path` -- The picture to look at.
///
/// # Returns
///
/// Its size and kind, or `None` when the file is missing, unreadable, or not
/// a picture this understands.
#[must_use]
pub fn read_image_info_from_file(path: &std::path::Path) -> Option<ImageInfo> {
    // Enough for a PNG header outright, and for the frame marker of every
    // JPEG met in practice, without reading a whole large picture into memory.
    const ENOUGH: usize = 64 * 1024;
    let bytes = std::fs::read(path).ok()?;
    read_image_info(&bytes[..bytes.len().min(ENOUGH)])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds the smallest PNG header that carries a size.
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        v.extend_from_slice(&13u32.to_be_bytes()); // chunk length
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&width.to_be_bytes());
        v.extend_from_slice(&height.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0]); // depth, colour type, etc.
        v
    }

    /// Builds a JPEG with one segment before the frame marker, so the walk is
    /// genuinely exercised rather than finding it immediately.
    fn jpeg_with_leading_segment(width: u16, height: u16) -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8]; // start of image
        // A short application segment, of the kind real JPEGs begin with.
        v.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x06, b'J', b'F', b'I', b'F']);
        // The frame marker.
        v.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
        v.extend_from_slice(&height.to_be_bytes());
        v.extend_from_slice(&width.to_be_bytes());
        v.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        v
    }

    #[test]
    fn reads_a_png_size_and_calls_it_png() {
        let info = read_image_info(&png_header(3000, 3000)).expect("should read a PNG");
        assert_eq!((info.width, info.height), (3000, 3000));
        assert_eq!(info.extension, "png");
    }

    #[test]
    fn reads_a_jpeg_size_and_calls_it_jpg() {
        let info = read_image_info(&jpeg_with_leading_segment(1400, 1400))
            .expect("should read a JPEG");
        assert_eq!((info.width, info.height), (1400, 1400));
        assert_eq!(info.extension, "jpg");
    }

    #[test]
    fn a_non_square_picture_keeps_its_two_dimensions_the_right_way_round() {
        // Easy to swap: JPEG records height before width.
        let info = read_image_info(&jpeg_with_leading_segment(1600, 900)).unwrap();
        assert_eq!((info.width, info.height), (1600, 900));

        let info = read_image_info(&png_header(1600, 900)).unwrap();
        assert_eq!((info.width, info.height), (1600, 900));
    }

    #[test]
    fn pictures_are_compared_by_total_pixels() {
        let wide = read_image_info(&png_header(4000, 500)).unwrap();
        let square = read_image_info(&png_header(1500, 1500)).unwrap();
        // Wider, but fewer pixels — which is the comparison that matters.
        assert!(wide.pixels() < square.pixels());
    }

    #[test]
    fn anything_that_is_not_a_picture_is_reported_as_unknown() {
        assert!(read_image_info(b"").is_none());
        assert!(read_image_info(b"not a picture at all").is_none());
        // A PNG signature with nothing after it.
        assert!(read_image_info(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]).is_none());
        // A JPEG start marker and nothing else.
        assert!(read_image_info(&[0xFF, 0xD8]).is_none());
    }

    #[test]
    fn a_zero_sized_picture_is_not_accepted() {
        // Would otherwise compare as "no pixels" and lose to everything,
        // which is right, but it is not a real picture and should be refused.
        assert!(read_image_info(&png_header(0, 100)).is_none());
        assert!(read_image_info(&jpeg_with_leading_segment(0, 100)).is_none());
    }

    #[test]
    fn a_malformed_jpeg_does_not_spin_for_ever() {
        // A segment claiming a length of zero would never move the walk
        // forward. Bounded rather than trusted.
        let bad = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x00, 0xFF, 0xC0];
        assert!(read_image_info(&bad).is_none());
    }

    #[test]
    fn reads_from_a_file_too() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cover.png");
        std::fs::write(&path, png_header(2000, 2000)).unwrap();
        let info = read_image_info_from_file(&path).expect("should read the file");
        assert_eq!(info.pixels(), 4_000_000);
    }

    #[test]
    fn a_missing_file_is_reported_as_unknown_rather_than_failing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_image_info_from_file(&dir.path().join("nothing.png")).is_none());
    }
}
