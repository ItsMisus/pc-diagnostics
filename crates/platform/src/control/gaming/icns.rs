//! Apple icon files (`.icns`): a container of typed entries. Modern icons store their large
//! sizes (`ic07`…`ic14`) as plain PNG data, which is all the app needs; JPEG 2000 and raw
//! ARGB entries are ignored.

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// The largest PNG entry of an `.icns` not bigger than `max_bytes`.
pub fn largest_png(data: &[u8], max_bytes: usize) -> Option<&[u8]> {
    if data.len() < 8 || &data[..4] != b"icns" {
        return None;
    }
    let total = usize::try_from(u32::from_be_bytes(data[4..8].try_into().ok()?)).ok()?;
    let end = total.min(data.len());
    let mut at = 8;
    let mut best: Option<&[u8]> = None;
    while at + 8 <= end {
        let len =
            usize::try_from(u32::from_be_bytes(data[at + 4..at + 8].try_into().ok()?)).ok()?;
        if len < 8 || at + len > end {
            break;
        }
        let body = &data[at + 8..at + len];
        if body.starts_with(PNG_MAGIC)
            && body.len() <= max_bytes
            && best.is_none_or(|b| body.len() > b.len())
        {
            best = Some(body);
        }
        at += len;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = kind.to_vec();
        v.extend_from_slice(&u32::try_from(body.len() + 8).expect("len").to_be_bytes());
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn picks_the_largest_png_under_the_cap() {
        let small = [PNG_MAGIC, b"small"].concat();
        let big = [PNG_MAGIC, &[0u8; 64]].concat();
        let mut body = entry(b"ic07", &small);
        body.extend(entry(b"ic04", b"ARGBxxxx"));
        body.extend(entry(b"ic10", &big));
        let mut file = b"icns".to_vec();
        file.extend_from_slice(&u32::try_from(body.len() + 8).expect("len").to_be_bytes());
        file.extend(body);

        assert_eq!(largest_png(&file, 1 << 20), Some(big.as_slice()));
        assert_eq!(largest_png(&file, 20), Some(small.as_slice()));
        assert_eq!(largest_png(b"nope", 100), None);
        assert_eq!(largest_png(&file[..20], 100), None);
    }
}
