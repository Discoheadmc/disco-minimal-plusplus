//! Disco Minimal++ teması — logo gradyanından türetilmiş koyu palet.
//! Kilometre Taşı 2'de 8 renkli tema seçici buraya yerleşecek.

use freya::prelude::*;

pub fn bg() -> Color {
    Color::from_rgb(15, 17, 32)
}

pub fn surface() -> Color {
    Color::from_rgb(27, 31, 58)
}

pub fn surface_raised() -> Color {
    Color::from_rgb(38, 43, 80)
}

pub fn surface_pressed() -> Color {
    Color::from_rgb(46, 52, 95)
}

pub fn accent() -> Color {
    Color::from_rgb(124, 92, 255)
}

pub fn accent_hover() -> Color {
    Color::from_rgb(96, 70, 230)
}

pub fn accent_pressed() -> Color {
    Color::from_rgb(82, 58, 210)
}

pub fn fg() -> Color {
    Color::from_rgb(240, 242, 250)
}

pub fn fg_muted() -> Color {
    Color::from_rgb(150, 156, 180)
}

pub fn danger() -> Color {
    Color::from_rgb(255, 92, 110)
}

pub fn success() -> Color {
    Color::from_rgb(0, 255, 163)
}
