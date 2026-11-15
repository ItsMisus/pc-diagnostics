//! Windows icon files (`.ico`): a directory of images. Since Windows Vista the 256 px entry
//! is stored as plain PNG data, which is all the app needs; BMP entries are ignored.

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// The largest PNG entry of an `.ico` not bigger than `max_bytes`.
pub fn largest_png(data: &[u8], max_bytes: usize) -> Option<&[u8]> {
    // ICONDIR: reserved 0, type 1 (icon), image count.
    if data.len() < 6 || data[0..4] != [0, 0, 1, 0] {
        return None;
    }
    let count = usize::from(u16::from_le_bytes([data[4], data[5]]));
    let mut best: Option<&[u8]> = None;
    for i in 0..count {
        // ICONDIRENTRY (16 bytes): size of the data at 8, its offset at 12.
        let at = 6 + i * 16;
        let Some(entry) = data.get(at..at + 16) else {
            break;
        };
        let len = usize::try_from(u32::from_le_bytes(entry[8..12].try_into().ok()?)).ok()?;
        let off = usize::try_from(u32::from_le_bytes(entry[12..16].try_into().ok()?)).ok()?;
        let Some(body) = off.checked_add(len).and_then(|end| data.get(off..end)) else {
            continue;
        };
        if body.starts_with(PNG_MAGIC)
            && body.len() <= max_bytes
            && best.is_none_or(|b| body.len() > b.len())
        {
            best = Some(body);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ico(bodies: &[&[u8]]) -> Vec<u8> {
        let count = u16::try_from(bodies.len()).expect("count");
        let mut head = vec![0, 0, 1, 0];
        head.extend_from_slice(&count.to_le_bytes());
        let mut off = 6 + bodies.len() * 16;
        let mut data = Vec::new();
        for b in bodies {
            head.extend_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0]);
            head.extend_from_slice(&u32::try_from(b.len()).expect("len").to_le_bytes());
            head.extend_from_slice(&u32::try_from(off).expect("off").to_le_bytes());
            off += b.len();
            data.extend_from_slice(b);
        }
        head.extend(data);
        head
    }

    #[test]
    fn picks_the_largest_png_under_the_cap() {
        let small = [PNG_MAGIC, b"small"].concat();
        let big = [PNG_MAGIC, &[0u8; 64]].concat();
        let bmp = [b"(\0\0\0".as_slice(), &[0u8; 200]].concat();
        let file = ico(&[&small, &bmp, &big]);

        assert_eq!(largest_png(&file, 1 << 20), Some(big.as_slice()));
        assert_eq!(largest_png(&file, 20), Some(small.as_slice()));
        assert_eq!(largest_png(&ico(&[&bmp]), 1 << 20), None);
        assert_eq!(largest_png(b"nope!!", 100), None);
        // Truncated: the entries point past the end.
        assert_eq!(largest_png(&file[..60], 1 << 20), None);
    }
}
