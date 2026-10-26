use crate::{db::Entry, kind, ocr};
use std::borrow::Cow;

const THUMB_MAX: u32 = 160;

pub struct Limits {
    pub max_chars: usize,
    pub max_image_pixels: u64,
    pub images: bool,
    pub files: bool,
    pub ocr: bool,
}

pub fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3))
}

pub fn text_entry(text: &str, limits: &Limits) -> Option<Entry> {
    if text.trim().is_empty() || text.chars().count() > limits.max_chars {
        return None;
    }
    Some(Entry {
        key: format!("t:{:016x}", fnv(text.as_bytes())),
        kind: kind::detect(text).as_str().into(),
        text: text.into(),
        ocr_text: String::new(),
        data: None,
        thumb: None,
        width: None,
        height: None,
        bytes: text.len() as i64,
    })
}

pub fn files_entry(paths: &[String]) -> Option<Entry> {
    if paths.is_empty() {
        return None;
    }
    let text = paths.join("\n");
    let bytes = paths.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len() as i64).sum();
    Some(Entry {
        key: format!("f:{:016x}", fnv(text.as_bytes())),
        kind: "file".into(),
        text,
        ocr_text: String::new(),
        data: None,
        thumb: None,
        width: None,
        height: None,
        bytes,
    })
}

pub fn image_entry(width: u32, height: u32, rgba: &[u8], limits: &Limits) -> Option<Entry> {
    let pixels = width as u64 * height as u64;
    if pixels == 0 || pixels > limits.max_image_pixels || rgba.len() as u64 != pixels * 4 {
        return None;
    }
    let png = encode_png(width, height, rgba)?;
    let (tw, th, small) = thumbnail(width, height, rgba, THUMB_MAX);
    let ocr_text = if limits.ocr { ocr::run(width, height, rgba).unwrap_or_default() } else { String::new() };
    Some(Entry {
        key: format!("i:{:016x}", fnv(rgba)),
        kind: "image".into(),
        text: String::new(),
        ocr_text,
        bytes: png.len() as i64,
        thumb: encode_png(tw, th, &small),
        data: Some(png),
        width: Some(width),
        height: Some(height),
    })
}

pub fn thumbnail(w: u32, h: u32, rgba: &[u8], max: u32) -> (u32, u32, Vec<u8>) {
    let scale = (max as f64 / w.max(h) as f64).min(1.0);
    let tw = ((w as f64 * scale).round() as u32).max(1);
    let th = ((h as f64 * scale).round() as u32).max(1);
    let mut out = Vec::with_capacity((tw * th * 4) as usize);
    for ty in 0..th {
        let (y0, y1) = (ty * h / th, ((ty + 1) * h / th).max(ty * h / th + 1));
        for tx in 0..tw {
            let (x0, x1) = (tx * w / tw, ((tx + 1) * w / tw).max(tx * w / tw + 1));
            let mut acc = [0u64; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = ((y * w + x) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += rgba[i + c] as u64;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u64;
            out.extend(acc.iter().map(|v| (v / n) as u8));
        }
    }
    (tw, th, out)
}

pub fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    enc.write_header().ok()?.write_image_data(rgba).ok()?;
    Some(out)
}

pub fn decode_png(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(data);
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        _ => return None,
    };
    Some((info.width, info.height, rgba))
}

pub fn read(cb: &mut arboard::Clipboard, limits: &Limits) -> Option<Entry> {
    if limits.files {
        if let Ok(list) = cb.get().file_list() {
            let paths: Vec<String> = list.iter().map(|p| p.to_string_lossy().into_owned()).collect();
            if let Some(e) = files_entry(&paths) {
                return Some(e);
            }
        }
    }
    if let Ok(text) = cb.get_text() {
        return text_entry(&text, limits);
    }
    if limits.images {
        if let Ok(img) = cb.get_image() {
            return image_entry(img.width as u32, img.height as u32, &img.bytes, limits);
        }
    }
    None
}

pub fn write(cb: &mut arboard::Clipboard, kind: &str, text: &str, data: Option<&[u8]>) -> Result<(), String> {
    match kind {
        "image" => {
            let (w, h, rgba) = data.and_then(decode_png).ok_or("stored image is unreadable")?;
            cb.set_image(arboard::ImageData { width: w as usize, height: h as usize, bytes: Cow::Owned(rgba) })
        }
        "file" => cb.set().file_list(&text.lines().collect::<Vec<_>>()),
        _ => cb.set_text(text),
    }
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: Limits = Limits { max_chars: 100, max_image_pixels: 1_000_000, images: true, files: true, ocr: false };

    #[test]
    fn png_round_trip_and_thumbnail() {
        let (w, h) = (400, 200);
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [(i % 256) as u8, 10, 20, 255]).collect();
        let e = image_entry(w, h, &rgba, &LIMITS).unwrap();
        assert_eq!(decode_png(e.data.as_ref().unwrap()).unwrap(), (w, h, rgba.clone()));
        let (tw, th, _) = decode_png(e.thumb.as_ref().unwrap()).unwrap();
        assert_eq!((tw, th), (160, 80));
        assert!(e.ocr_text.is_empty());
        assert_eq!(e.key, image_entry(w, h, &rgba, &LIMITS).unwrap().key);
    }

    #[test]
    fn rejects_oversized_or_empty() {
        assert!(text_entry("   \n", &LIMITS).is_none());
        assert!(text_entry(&"x".repeat(101), &LIMITS).is_none());
        assert!(image_entry(2000, 2000, &vec![0; 2000 * 2000 * 4], &LIMITS).is_none());
        assert!(files_entry(&[]).is_none());
    }

    #[test]
    fn files_are_one_entry_per_copy() {
        let e = files_entry(&[r"C:\a.txt".into(), r"C:\b.png".into()]).unwrap();
        assert_eq!((e.kind.as_str(), e.text.as_str()), ("file", "C:\\a.txt\nC:\\b.png"));
        assert_eq!(text_entry("https://x.dev", &LIMITS).unwrap().kind, "link");
    }
}
