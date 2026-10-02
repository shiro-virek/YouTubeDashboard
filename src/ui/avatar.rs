//! Circular channel avatars drawn on a canvas: a colour derived from the
//! channel name plus its initial.

use std::f64::consts::PI;

use gtk::cairo;
use gtk::prelude::*;

/// Deterministic hue (0..360) for a name, so a channel always keeps its color.
pub fn hue_of(text: &str) -> f64 {
    // FNV-1a: cheap, stable and good enough to spread names over the wheel.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (hash % 360) as f64
}

fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> (f64, f64, f64) {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue / 60.0;
    let second = chroma * (1.0 - (sector % 2.0 - 1.0).abs());

    let (red, green, blue) = match sector as u32 % 6 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };

    let offset = lightness - chroma / 2.0;
    (red + offset, green + offset, blue + offset)
}

/// A fixed size round avatar with a single letter.
pub fn build(seed: &str, initial: &str, size: i32) -> gtk::DrawingArea {
    let initial = initial.to_string();
    let hue = hue_of(seed);

    let area = gtk::DrawingArea::builder()
        .content_width(size)
        .content_height(size)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Start)
        .build();

    area.set_draw_func(move |_, cr, width, height| {
        let diameter = width.min(height) as f64;
        if diameter <= 2.0 {
            return;
        }

        let center_x = width as f64 / 2.0;
        let center_y = height as f64 / 2.0;
        let radius = diameter / 2.0 - 1.0;

        let (red, green, blue) = hsl_to_rgb(hue, 0.5, 0.45);
        cr.set_source_rgb(red, green, blue);
        cr.arc(center_x, center_y, radius, 0.0, 2.0 * PI);
        let _ = cr.fill();

        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        cr.set_font_size(diameter * 0.46);

        if let Ok(extents) = cr.text_extents(&initial) {
            cr.move_to(
                center_x - extents.width() / 2.0 - extents.x_bearing(),
                center_y - extents.height() / 2.0 - extents.y_bearing(),
            );
            let _ = cr.show_text(&initial);
        }
    });

    area
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_is_stable_and_in_range() {
        assert_eq!(hue_of("Rust"), hue_of("Rust"));
        assert!((0.0..360.0).contains(&hue_of("Rust")));
    }

    #[test]
    fn hues_differ_between_names() {
        assert_ne!(hue_of("Rust"), hue_of("Python"));
    }

    #[test]
    fn hsl_conversion_stays_in_gamut() {
        for hue in [0.0, 60.0, 120.0, 240.0, 359.0] {
            let (r, g, b) = hsl_to_rgb(hue, 0.5, 0.45);
            for value in [r, g, b] {
                assert!((0.0..=1.0).contains(&value), "hue {hue} produced {value}");
            }
        }
        // Pure red at hue 0.
        assert_eq!(hsl_to_rgb(0.0, 1.0, 0.5), (1.0, 0.0, 0.0));
        // White and black are the achromatic ends.
        assert_eq!(hsl_to_rgb(123.0, 0.0, 1.0), (1.0, 1.0, 1.0));
        assert_eq!(hsl_to_rgb(123.0, 0.0, 0.0), (0.0, 0.0, 0.0));
    }
}
