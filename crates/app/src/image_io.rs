//! 把常见图像读成 RGBA，并压成可以放进项目文件的 PNG。

use base64::Engine;
use image::ImageEncoder;

pub struct DecodedRgba {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn decode_rgba(bytes: &[u8]) -> Result<DecodedRgba, String> {
    let image = image::load_from_memory(bytes).map_err(|err| {
        sciplot_core::trf!("无法读取图像：{err}", "Could not read the image: {err}", err = err)
    })?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(DecodedRgba {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

pub fn png_base64(width: u32, height: u32, rgba: &[u8]) -> Result<String, String> {
    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    encoder
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|err| sciplot_core::trf!("无法编码 PNG：{err}", "Could not encode PNG: {err}", err = err))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(buf))
}

/// 长边超过 `max_side` 时缩小，避免图上的图像把项目文件撑得太大。
pub fn fit_side(image: &DecodedRgba, max_side: u32) -> Result<DecodedRgba, String> {
    if image.width <= max_side && image.height <= max_side {
        return Ok(DecodedRgba {
            width: image.width,
            height: image.height,
            rgba: image.rgba.clone(),
        });
    }
    let scale = max_side as f32 / image.width.max(image.height) as f32;
    let width = ((image.width as f32) * scale).round().max(1.0) as u32;
    let height = ((image.height as f32) * scale).round().max(1.0) as u32;
    let source = image::RgbaImage::from_raw(image.width, image.height, image.rgba.clone())
        .ok_or_else(|| sciplot_core::tr!("图像数据不完整", "The image buffer is incomplete").to_string())?;
    let scaled = image::imageops::resize(&source, width, height, image::imageops::FilterType::Triangle);
    Ok(DecodedRgba {
        width,
        height,
        rgba: scaled.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_roundtrip_keeps_a_red_pixel() {
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        encoder
            .write_image(&[255, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let decoded = decode_rgba(&buf).unwrap();
        assert_eq!(decoded.width, 1);
        assert_eq!(decoded.rgba, vec![255, 0, 0, 255]);
        let text = png_base64(decoded.width, decoded.height, &decoded.rgba).unwrap();
        assert!(text.starts_with("iVBOR"));
    }
}
