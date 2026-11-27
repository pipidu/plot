use std::io::Cursor;

use base64::Engine;

pub struct DecodedPng {
    pub rgba: Vec<u8>,
    pub cols: u32,
    pub rows: u32,
}

pub fn decode_png_base64(text: &str) -> Option<DecodedPng> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(text.trim()).ok()?;
    decode_png(&bytes)
}

pub fn decode_png(bytes: &[u8]) -> Option<DecodedPng> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    let data = &buffer[..info.buffer_size()];
    let cols = info.width;
    let rows = info.height;
    if cols == 0 || rows == 0 {
        return None;
    }
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks(3)
            .flat_map(|px| [px[0], px[1], px[2], 255])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|v| [*v, *v, *v, 255]).collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks(2)
            .flat_map(|px| [px[0], px[0], px[0], px[1]])
            .collect(),
        _ => return None,
    };
    let expected = cols as usize * rows as usize * 4;
    if rgba.len() < expected {
        return None;
    }
    Some(DecodedPng {
        rgba: rgba[..expected].to_vec(),
        cols,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_one_red_pixel() {
        let mut encoded = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut encoded, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[200, 10, 10, 255]).unwrap();
        }
        let text = base64::engine::general_purpose::STANDARD.encode(&encoded);
        let decoded = decode_png_base64(&text).unwrap();
        assert_eq!(decoded.cols, 1);
        assert_eq!(decoded.rgba, vec![200, 10, 10, 255]);
    }
}
