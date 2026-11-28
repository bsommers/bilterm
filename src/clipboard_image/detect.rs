//! Magic-byte sniffing and header-only dimension parsing for clipboard images.
//!
//! Nothing here decodes pixels: only the first few KiB of an untrusted payload are inspected,
//! so a hostile "decompression bomb" costs one header read. The caller applies the dimension caps
//! before the bytes ever reach disk.

use super::ImageError;

/// Image container formats Bilterm will store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Gif,
    Webp,
    Bmp,
}

impl ImageKind {
    /// File extension used for the stored file (never taken from the payload).
    pub fn ext(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
        }
    }

    /// IANA media type.
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Bmp => "image/bmp",
        }
    }
}

/// Result of sniffing one payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageInfo {
    pub kind: ImageKind,
    pub width: u32,
    pub height: u32,
}

fn le16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?])))
}

fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(at)?, *b.get(at + 1)?, *b.get(at + 2)?, *b.get(at + 3)?]))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*b.get(at)?, *b.get(at + 1)?, *b.get(at + 2)?, *b.get(at + 3)?]))
}

fn le24(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(*b.get(at)?) | u32::from(*b.get(at + 1)?) << 8 | u32::from(*b.get(at + 2)?) << 16)
}

fn bad(why: &str) -> ImageError {
    ImageError::NotAnImage(why.to_string())
}

fn png(b: &[u8]) -> Result<(u32, u32), ImageError> {
    if b.get(12..16) != Some(b"IHDR") {
        return Err(bad("PNG without IHDR"));
    }
    Ok((be32(b, 16).ok_or_else(|| bad("short PNG"))?, be32(b, 20).ok_or_else(|| bad("short PNG"))?))
}

fn gif(b: &[u8]) -> Result<(u32, u32), ImageError> {
    Ok((le16(b, 6).ok_or_else(|| bad("short GIF"))?, le16(b, 8).ok_or_else(|| bad("short GIF"))?))
}

fn bmp(b: &[u8]) -> Result<(u32, u32), ImageError> {
    let w = le32(b, 18).ok_or_else(|| bad("short BMP"))? as i32;
    let h = le32(b, 22).ok_or_else(|| bad("short BMP"))? as i32;
    Ok((w.unsigned_abs(), h.unsigned_abs()))
}

fn webp(b: &[u8]) -> Result<(u32, u32), ImageError> {
    let short = || bad("short WebP");
    match b.get(12..16) {
        Some(b"VP8X") => Ok((le24(b, 24).ok_or_else(short)? + 1, le24(b, 27).ok_or_else(short)? + 1)),
        Some(b"VP8 ") => {
            if b.get(23..26) != Some(&[0x9d, 0x01, 0x2a]) {
                return Err(bad("WebP VP8 without start code"));
            }
            Ok((le16(b, 26).ok_or_else(short)? & 0x3fff, le16(b, 28).ok_or_else(short)? & 0x3fff))
        }
        Some(b"VP8L") => {
            if b.get(20) != Some(&0x2f) {
                return Err(bad("WebP VP8L without signature"));
            }
            let bits = le32(b, 21).ok_or_else(short)?;
            Ok(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
        }
        _ => Err(bad("unknown WebP chunk")),
    }
}

fn jpeg(b: &[u8]) -> Result<(u32, u32), ImageError> {
    let mut i = 2usize;
    // Walk marker segments; a SOFn frame header carries the dimensions.
    for _ in 0..4096 {
        while b.get(i) == Some(&0xff) && b.get(i + 1) == Some(&0xff) {
            i += 1;
        }
        if b.get(i) != Some(&0xff) {
            return Err(bad("corrupt JPEG marker stream"));
        }
        let m = *b.get(i + 1).ok_or_else(|| bad("truncated JPEG"))?;
        if m == 0xd8 || m == 0x01 || (0xd0..=0xd7).contains(&m) {
            i += 2;
            continue;
        }
        let len = u32::from(u16::from_be_bytes([
            *b.get(i + 2).ok_or_else(|| bad("truncated JPEG"))?,
            *b.get(i + 3).ok_or_else(|| bad("truncated JPEG"))?,
        ])) as usize;
        if matches!(m, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            let h = u32::from(u16::from_be_bytes([*b.get(i + 5).ok_or_else(|| bad("short JPEG"))?, *b.get(i + 6).ok_or_else(|| bad("short JPEG"))?]));
            let w = u32::from(u16::from_be_bytes([*b.get(i + 7).ok_or_else(|| bad("short JPEG"))?, *b.get(i + 8).ok_or_else(|| bad("short JPEG"))?]));
            return Ok((w, h));
        }
        if m == 0xda || len < 2 {
            return Err(bad("JPEG without frame header"));
        }
        i += 2 + len;
    }
    Err(bad("JPEG header too long"))
}

/// Identifies `bytes` by magic number and reads its pixel dimensions from the header.
pub fn sniff(bytes: &[u8]) -> Result<ImageInfo, ImageError> {
    let (kind, dims) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        (ImageKind::Png, png(bytes)?)
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        (ImageKind::Jpeg, jpeg(bytes)?)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        (ImageKind::Gif, gif(bytes)?)
    } else if bytes.len() > 16 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        (ImageKind::Webp, webp(bytes)?)
    } else if bytes.starts_with(b"BM") {
        (ImageKind::Bmp, bmp(bytes)?)
    } else {
        return Err(bad("unrecognised image format"));
    };
    if dims.0 == 0 || dims.1 == 0 {
        return Err(bad("zero-sized image"));
    }
    Ok(ImageInfo { kind, width: dims.0, height: dims.1 })
}

#[cfg(test)]
pub(crate) mod fixtures {
    /// Minimal PNG header (signature, IHDR) of the given size; not decodable, only sniffable.
    pub fn png(w: u32, h: u32) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::png;
    use super::*;

    #[test]
    fn png_dimensions() {
        let i = sniff(&png(640, 480)).unwrap();
        assert_eq!((i.kind, i.width, i.height), (ImageKind::Png, 640, 480));
    }

    #[test]
    fn gif_bmp_webp_jpeg_dimensions() {
        let mut g = b"GIF89a".to_vec();
        g.extend_from_slice(&[10, 0, 20, 0, 0, 0, 0]);
        let i = sniff(&g).unwrap();
        assert_eq!((i.kind, i.width, i.height), (ImageKind::Gif, 10, 20));

        let mut b = vec![0u8; 30];
        b[..2].copy_from_slice(b"BM");
        b[18..22].copy_from_slice(&7i32.to_le_bytes());
        b[22..26].copy_from_slice(&(-9i32).to_le_bytes());
        let i = sniff(&b).unwrap();
        assert_eq!((i.kind, i.width, i.height), (ImageKind::Bmp, 7, 9));

        let mut w = vec![0u8; 40];
        w[..4].copy_from_slice(b"RIFF");
        w[8..12].copy_from_slice(b"WEBP");
        w[12..16].copy_from_slice(b"VP8X");
        w[24..27].copy_from_slice(&[99, 0, 0]);
        w[27..30].copy_from_slice(&[49, 0, 0]);
        let i = sniff(&w).unwrap();
        assert_eq!((i.kind, i.width, i.height), (ImageKind::Webp, 100, 50));

        // SOI, APP0 (len 4), SOF0 (h=300,w=200)
        let j = [
            0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0, 0, 0xff, 0xc0, 0, 11, 8, 0x01, 0x2c, 0x00, 0xc8, 3, 0, 0,
        ];
        let i = sniff(&j).unwrap();
        assert_eq!((i.kind, i.width, i.height), (ImageKind::Jpeg, 200, 300));
    }

    #[test]
    fn rejects_garbage_truncated_and_zero() {
        assert!(sniff(b"hello world, not an image").is_err());
        assert!(sniff(b"").is_err());
        assert!(sniff(b"\x89PNG\r\n\x1a\n").is_err());
        assert!(sniff(&png(0, 5)).is_err());
        assert!(sniff(&[0xff, 0xd8, 0xff]).is_err());
        let mut hostile = vec![0xff, 0xd8];
        for _ in 0..5000 {
            hostile.extend_from_slice(&[0xff, 0xe0, 0, 2]);
        }
        assert!(sniff(&hostile).is_err());
    }
}
