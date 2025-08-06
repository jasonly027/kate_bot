use image::{ImageBuffer, Luma};
use rusttype::{Font, Scale, point};
use std::{
    fmt::{Debug, Display},
    process,
    str::FromStr,
};
use std::{io::Cursor, sync::LazyLock};
use tracing::{error, warn};

pub trait ParseUnwrapAll<T> {
    fn parse_unwrap_all(self) -> Vec<T>;
}

impl<I, T> ParseUnwrapAll<T> for I
where
    I: IntoIterator,
    I::Item: AsRef<str>,
    T: FromStr,
    T::Err: Debug,
{
    fn parse_unwrap_all(self) -> Vec<T> {
        self.into_iter()
            .map(|value| {
                value.as_ref().parse().unwrap_or_else(|err| {
                    error!(message = "Failed to parse value", value = value.as_ref(), error = ?err);
                    process::exit(2)
                })
            })
            .collect()
    }
}

pub fn log_sending_error<E: Display>(err: &E) {
    warn!(message = "Send failed", error = %err);
}

/// Converts `text` into a rasterized PNG image in bytes.
pub fn text_to_image(text: &str) -> Vec<u8> {
    static FONT: LazyLock<Font<'static>> = LazyLock::new(|| {
        static FONT_DATA: &[u8; 5728064] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fonts/NotoSansJPBold.ttf"
        ));

        Font::try_from_bytes(FONT_DATA).unwrap()
    });

    let scale = Scale::uniform(72.0);
    let v_metrics = FONT.v_metrics(scale);

    const PADDING: f32 = 60.0;

    let glyphs: Vec<_> = FONT
        .layout(text, scale, point(PADDING, PADDING + v_metrics.ascent))
        .collect();

    let glyphs_height = (v_metrics.ascent - v_metrics.descent).ceil() as u32;
    let glyphs_width = {
        let min_x = glyphs
            .first()
            .map(|g| g.pixel_bounding_box().unwrap().min.x)
            .unwrap();
        let max_x = glyphs
            .last()
            .map(|g| g.pixel_bounding_box().unwrap().max.x)
            .unwrap();
        (max_x - min_x) as u32
    };

    let mut image = ImageBuffer::<Luma<u8>, Vec<u8>>::from_pixel(
        glyphs_width + (PADDING * 2.0) as u32,
        glyphs_height + (PADDING * 2.0) as u32,
        Luma([255]),
    );

    for glyph in glyphs {
        if let Some(bounding_box) = glyph.pixel_bounding_box() {
            glyph.draw(|x, y, v| {
                image.put_pixel(
                    // Offset the position by the glyph bounding box
                    x + bounding_box.min.x as u32,
                    y + bounding_box.min.y as u32,
                    Luma([255 - (v * 255.0) as u8]),
                )
            });
        }
    }

    let mut buf = Cursor::new(Vec::new());
    image.write_to(&mut buf, image::ImageFormat::Png).unwrap();

    buf.into_inner()
}
