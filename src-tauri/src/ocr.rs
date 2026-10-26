// Image text recognition through the OCR engine built into Windows 10/11.
// Everything runs locally; no image ever leaves the machine.

#[cfg(windows)]
pub fn run(width: u32, height: u32, rgba: &[u8]) -> Option<String> {
    use std::cell::Cell;
    use windows::core::Interface;
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::Buffer;
    use windows::Win32::System::WinRT::{IBufferByteAccess, RoInitialize, RO_INIT_MULTITHREADED};

    thread_local! {
        static RO_INIT: Cell<bool> = Cell::new(false);
    }

    RO_INIT.with(|init| {
        if !init.get() {
            unsafe {
                let _ = RoInitialize(RO_INIT_MULTITHREADED);
            }
            init.set(true);
        }
    });

    if width == 0 || height == 0 || rgba.is_empty() {
        return None;
    }

    // WinRT wants BGRA, clipboard images arrive as RGBA.
    let mut bgra = rgba.to_vec();
    for chunk in bgra.chunks_mut(4) {
        chunk.swap(0, 2);
    }
    let len = bgra.len();

    let buffer = Buffer::Create(len as u32).ok()?;
    unsafe {
        let byte_access: IBufferByteAccess = buffer.cast().ok()?;
        let p = byte_access.Buffer().ok()?;
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), p, len);
    }
    let _ = buffer.SetLength(len as u32);

    let bitmap =
        SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, width as i32, height as i32)
            .ok()?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok()?;
    let result = engine.RecognizeAsync(&bitmap).ok()?.get().ok()?;
    let text = result.Text().ok()?.to_string();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(if trimmed.chars().count() > 5000 {
        trimmed.chars().take(5000).collect()
    } else {
        trimmed.to_string()
    })
}

#[cfg(not(windows))]
pub fn run(_width: u32, _height: u32, _rgba: &[u8]) -> Option<String> {
    None
}
