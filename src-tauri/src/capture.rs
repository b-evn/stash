use crate::{db::Entry, kind, ocr};
use std::borrow::Cow;

const THUMB_MAX: u32 = 160;
/// Largest file stash reads to draw a thumbnail of it.
const MAX_VISUAL_BYTES: u64 = 20 * 1024 * 1024;
/// File types the webview can draw, so they get a thumbnail.
const VISUAL_EXTS: &[&str] = &["svg", "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "avif", "heic", "heif"];

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

fn is_visual_path(path: &str) -> bool {
    std::path::Path::new(path.trim())
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VISUAL_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Whether text is a whole SVG document, as design tools and icon sites copy it.
pub fn is_svg(text: &str) -> bool {
    let mut t = text.trim();
    if !t.ends_with("</svg>") {
        return false;
    }
    // Skip the XML declaration, comments and doctype a document may start with.
    loop {
        let close = if t.starts_with("<?") { "?>" } else if t.starts_with("<!--") { "-->" } else if t.starts_with("<!") { ">" } else { break };
        match t.find(close) {
            Some(i) => t = t[i + close.len()..].trim_start(),
            None => return false,
        }
    }
    t.strip_prefix("<svg").is_some_and(|rest| rest.starts_with(|c: char| c.is_whitespace() || c == '>'))
}

/// The bytes a clip's thumbnail is drawn from: the first image among copied
/// files, or copied SVG markup. Image clips have their thumbnail already.
pub fn visual_source(kind: &str, text: &str) -> Option<Vec<u8>> {
    match kind {
        "image" => None,
        "file" => {
            let path = text.lines().map(str::trim).find(|p| is_visual_path(p))?;
            let meta = std::fs::metadata(path).ok()?;
            if !meta.is_file() || meta.len() > MAX_VISUAL_BYTES {
                return None;
            }
            std::fs::read(path).ok()
        }
        _ => is_svg(text).then(|| text.trim().as_bytes().to_vec()),
    }
}

/// A thumbnail the webview drew, as a PNG or JPEG data URL, checked and decoded.
pub fn thumb_from_data_url(url: &str) -> Option<Vec<u8>> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let b64 = url.strip_prefix("data:image/png;base64,").or_else(|| url.strip_prefix("data:image/jpeg;base64,"))?;
    let bytes = STANDARD.decode(b64).ok()?;
    let known = bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(&[0xFF, 0xD8, 0xFF]);
    (known && bytes.len() <= 128 * 1024).then_some(bytes)
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
    fn svg_markup_is_recognized() {
        assert!(is_svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M1 1"/></svg>"#));
        assert!(is_svg("<?xml version=\"1.0\"?>\n<!-- Generator: Illustrator -->\n<!DOCTYPE svg>\n<svg>\n</svg>\n"));
        assert!(!is_svg("<svgfoo></svg>"));
        assert!(!is_svg("<div><svg></svg></div>"));
        assert!(!is_svg("<svg viewBox=\"0 0 1 1\">"));
        assert_eq!(visual_source("code", " <svg></svg> "), Some(b"<svg></svg>".to_vec()));
        assert_eq!(visual_source("text", "hello"), None);
    }

    #[test]
    fn image_files_are_read_for_thumbnails() {
        let dir = std::env::temp_dir().join(format!("stash-visual-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (txt, png) = (dir.join("notes.txt"), dir.join("Logo.PNG"));
        std::fs::write(&txt, b"hi").unwrap();
        std::fs::write(&png, b"png bytes").unwrap();
        let list = format!("{}\n{}", txt.display(), png.display());
        assert_eq!(visual_source("file", &list), Some(b"png bytes".to_vec()));
        assert_eq!(visual_source("file", &txt.display().to_string()), None);
        assert_eq!(visual_source("file", &dir.join("gone.png").display().to_string()), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn thumbs_from_the_webview_are_checked() {
        let png = encode_png(1, 1, &[0, 0, 0, 0]).unwrap();
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let url = format!("data:image/png;base64,{}", STANDARD.encode(&png));
        assert_eq!(thumb_from_data_url(&url), Some(png));
        assert_eq!(thumb_from_data_url("data:image/jpeg;base64,/9j/4AAQ"), Some(vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]));
        assert_eq!(thumb_from_data_url("data:image/png;base64,aGVsbG8="), None);
        assert_eq!(thumb_from_data_url("data:text/html;base64,aGVsbG8="), None);
    }

    #[test]
    fn files_are_one_entry_per_copy() {
        let e = files_entry(&[r"C:\a.txt".into(), r"C:\b.png".into()]).unwrap();
        assert_eq!((e.kind.as_str(), e.text.as_str()), ("file", "C:\\a.txt\nC:\\b.png"));
        assert_eq!(text_entry("https://x.dev", &LIMITS).unwrap().kind, "link");
    }
}
