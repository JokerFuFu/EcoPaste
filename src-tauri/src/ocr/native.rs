use std::path::Path;

#[cfg(target_os = "macos")]
const MAX_INPUT_BYTES: u64 = 20 * 1024 * 1024;
#[cfg(target_os = "macos")]
const MAX_PIXELS: u64 = 40_000_000;
#[cfg(target_os = "macos")]
const MAX_OUTPUT_CHARS: usize = 100_000;

pub fn supported() -> bool {
    cfg!(target_os = "macos")
}

#[cfg(target_os = "windows")]
pub fn recognize(_path: &Path) -> anyhow::Result<String> {
    anyhow::bail!("Local image OCR is not available on Windows")
}

/// Runs local Vision recognition synchronously; callers must use a blocking worker.
/// The immutable, bounded byte snapshot is also the image validated below.
#[cfg(target_os = "macos")]
pub fn recognize(path: &Path) -> anyhow::Result<String> {
    use anyhow::{ensure, Context};
    use objc2::{rc::autoreleasepool, AnyThread};
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
    };
    use std::io::{Cursor, Read};

    let metadata = std::fs::metadata(path).context("Cannot open OCR image")?;
    ensure!(metadata.is_file(), "OCR image must be a regular file");
    ensure!(
        metadata.len() <= MAX_INPUT_BYTES,
        "OCR image exceeds 20 MiB"
    );

    let file = std::fs::File::open(path).context("Cannot open OCR image")?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("Cannot read OCR image")?;
    ensure!(
        bytes.len() as u64 <= MAX_INPUT_BYTES,
        "OCR image exceeds 20 MiB"
    );

    let (width, height) = image::ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .context("Invalid OCR image format")?
        .into_dimensions()
        .context("Invalid OCR image dimensions")?;
    ensure!(width > 0 && height > 0, "OCR image is empty");
    ensure!(
        u64::from(width) * u64::from(height) <= MAX_PIXELS,
        "OCR image exceeds 40 megapixels"
    );

    autoreleasepool(|_| {
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setRecognitionLanguages(&NSArray::from_retained_slice(&[
            NSString::from_str("zh-Hans"),
            NSString::from_str("en-US"),
        ]));
        request.setUsesLanguageCorrection(true);

        let data = NSData::from_vec(bytes);
        let handler = VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &data,
            &NSDictionary::new(),
        );
        let requests = NSArray::<VNRequest>::from_slice(&[&request]);
        handler.performRequests_error(&requests).map_err(|error| {
            anyhow::anyhow!(
                "Apple Vision could not recognize this image (error {})",
                error.code()
            )
        })?;

        let observations = request
            .results()
            .context("Apple Vision returned no result")?;
        Ok(bounded_text(observations.iter().filter_map(
            |observation| {
                observation
                    .topCandidates(1)
                    .firstObject()
                    .map(|text| text.string().to_string())
            },
        )))
    })
}

/// Joins nonempty observations without splitting UTF-8 or exceeding the index limit.
#[cfg(target_os = "macos")]
fn bounded_text(lines: impl Iterator<Item = String>) -> String {
    let mut output = String::new();
    let mut remaining = MAX_OUTPUT_CHARS;
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !output.is_empty() {
            output.push('\n');
            remaining -= 1;
        }
        for character in line.chars().take(remaining) {
            output.push(character);
            remaining -= 1;
        }
        if remaining == 0 {
            break;
        }
    }
    output
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/ocr/native_fixtures")
            .join(name)
    }

    #[test]
    fn recognizes_generated_english_image() {
        assert!(supported());
        let result = recognize(&fixture("english.png")).unwrap();
        assert!(result.contains("EcoPaste local image search"));
    }

    #[test]
    fn recognizes_generated_chinese_image() {
        let result = recognize(&fixture("chinese.png")).unwrap();
        assert!(result.contains("本地图片文字识别测试"));
    }

    #[test]
    fn rejects_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let error = recognize(&dir.path().join("missing.png")).unwrap_err();
        assert!(error.to_string().contains("open"));
    }

    #[test]
    fn rejects_invalid_image() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"this is not an image").unwrap();
        let error = recognize(file.path()).unwrap_err();
        assert!(error.to_string().contains("image"));
    }

    #[test]
    fn rejects_file_above_twenty_mib() {
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(20 * 1024 * 1024 + 1).unwrap();
        let error = recognize(file.path()).unwrap_err();
        assert!(error.to_string().contains("20 MiB"));
    }

    #[test]
    fn rejects_image_above_forty_megapixels() {
        let file = tempfile::NamedTempFile::new().unwrap();
        image::GrayImage::new(6400, 6400)
            .save_with_format(file.path(), image::ImageFormat::Png)
            .unwrap();
        let error = recognize(file.path()).unwrap_err();
        assert!(error.to_string().contains("40 megapixels"));
    }

    #[test]
    fn blank_image_has_no_text() {
        let file = tempfile::NamedTempFile::new().unwrap();
        image::GrayImage::from_pixel(640, 480, image::Luma([255]))
            .save_with_format(file.path(), image::ImageFormat::Png)
            .unwrap();
        assert!(recognize(file.path()).unwrap().is_empty());
    }

    #[test]
    fn output_limit_counts_unicode_characters_and_separators() {
        let result = bounded_text(["文".repeat(99_998), "😀文".into()].into_iter());
        assert_eq!(result.chars().count(), 100_000);
        assert!(result.ends_with("\n😀"));
    }
}
