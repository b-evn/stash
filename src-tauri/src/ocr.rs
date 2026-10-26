// Image text recognition through the OCR engine built into Windows 10/11 or macOS.
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
    tidy(&result.Text().ok()?.to_string())
}

// The same through the Vision framework on macOS.
#[cfg(target_os = "macos")]
pub fn run(width: u32, height: u32, rgba: &[u8]) -> Option<String> {
    use objc2::{rc::autoreleasepool, sel, AnyThread};
    use objc2_core_foundation::CFData;
    use objc2_core_graphics::{
        CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo,
    };
    use objc2_foundation::{NSArray, NSDictionary, NSObjectProtocol};
    use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

    if width == 0 || height == 0 || rgba.is_empty() {
        return None;
    }
    autoreleasepool(|_| {
        let data = CFData::from_bytes(rgba);
        let provider = CGDataProvider::with_cf_data(Some(&data))?;
        let space = CGColorSpace::new_device_rgb()?;
        let image = unsafe {
            CGImage::new(
                width as usize,
                height as usize,
                8,
                32,
                width as usize * 4,
                Some(&space),
                CGBitmapInfo(CGImageAlphaInfo::Last.0),
                Some(&provider),
                std::ptr::null(),
                false,
                CGColorRenderingIntent::RenderingIntentDefault,
            )
        }?;

        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);
        if request.respondsToSelector(sel!(setAutomaticallyDetectsLanguage:)) {
            request.setAutomaticallyDetectsLanguage(true);
        }
        let handler = unsafe {
            VNImageRequestHandler::initWithCGImage_options(VNImageRequestHandler::alloc(), &image, &NSDictionary::new())
        };
        let requests = NSArray::from_slice(&[&**request as &VNRequest]);
        handler.performRequests_error(&requests).ok()?;
        let lines: Vec<String> = request
            .results()?
            .iter()
            .filter_map(|o| o.topCandidates(1).firstObject())
            .map(|c| c.string().to_string())
            .collect();
        tidy(&lines.join("\n"))
    })
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn run(_width: u32, _height: u32, _rgba: &[u8]) -> Option<String> {
    None
}

#[cfg(any(windows, target_os = "macos"))]
fn tidy(text: &str) -> Option<String> {
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
