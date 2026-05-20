use ratatui::style::Color;

#[allow(dead_code)]
pub struct Theme {
    pub bg: Color,
    pub bg_alt: Color,
    pub bg_deep: Color,
    pub fg: Color,
    pub fg_alt: Color,
    pub dim: Color,
    pub mute: Color,
    pub accent: Color,
    pub live: Color,
    pub warn: Color,
    pub danger: Color,
    pub soft: Color,
}

// TODO: gracefully distil colors when true-colors are not supported by the terminal

pub const PATINA: Theme = Theme {
    bg: Color::Rgb(22, 16, 12),
    bg_alt: Color::Rgb(32, 25, 20),
    bg_deep: Color::Rgb(12, 8, 6),
    fg: Color::Rgb(235, 231, 223),
    fg_alt: Color::Rgb(0xA9, 0x94, 0xD6),
    dim: Color::Rgb(128, 121, 113),
    mute: Color::Rgb(71, 65, 60),
    accent: Color::Rgb(233, 147, 85),
    live: Color::Rgb(111, 196, 160),
    warn: Color::Rgb(232, 170, 78),
    danger: Color::Rgb(236, 91, 87),
    soft: Color::Rgb(184, 174, 151),
};
