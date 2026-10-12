//! Yeniden kullanılabilir UI parçaları.

pub mod button;

pub use button::{ButtonVariant, DButton};

use freya::prelude::*;
use freya::radio::use_radio;

use crate::state::AppChannel;

/// Disco Launcher'daki 8 renk geleneği.
pub const ACCENTS: [&str; 8] = [
    "#7c5cff", // mor (varsayılan)
    "#00c2ff", // camgöbeği
    "#00ffa3", // nane yeşili
    "#ff5cc8", // pembe
    "#ff9f45", // turuncu
    "#ff5c5c", // kırmızı
    "#ffd23f", // sarı
    "#5c7cff", // mavi
];

/// Ayarlardaki vurgu rengi ve türevleri (hover/pressed koyulması).
pub struct AccentPalette {
    pub base: Color,
    pub hover: Color,
    pub pressed: Color,
}

/// Ayarlardaki vurgu rengini okuyan hook.
pub fn use_accent() -> AccentPalette {
    let settings = use_radio(AppChannel::Settings).read().settings.clone();
    accent_palette_of(&settings.accent)
}

/// "#rrggbb" → Color; bozuksa varsayılan mor.
pub fn hex_to_color(hex: &str) -> Color {
    accent_palette_of(hex).base
}

/// Verilen hex'ten (dinamik) buton paleti.
pub fn accent_palette_of(hex: &str) -> AccentPalette {
    let hex = hex.trim_start_matches('#');
    let mut values = [124u8, 92, 255];
    if hex.len() == 6 {
        for index in 0..3 {
            if let Ok(value) = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16) {
                values[index] = value;
            }
        }
    }
    let scale = |factor: f32| -> Color {
        Color::from_rgb(
            (values[0] as f32 * factor).min(255.) as u8,
            (values[1] as f32 * factor).min(255.) as u8,
            (values[2] as f32 * factor).min(255.) as u8,
        )
    };
    AccentPalette {
        base: scale(1.0),
        hover: scale(0.82),
        pressed: scale(0.66),
    }
}
