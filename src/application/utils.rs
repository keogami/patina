use std::{borrow::Cow, ops::Range, time::Duration};

use ratatui::{
    layout::Layout,
    macros::span,
    style::{Color, Style},
    widgets::Widget,
};

use crate::application::theme::PATINA;

/// Tracks scrolling state for viewport continuity.
#[derive(Debug, Clone, Copy)]
pub struct ScrollState {
    /// The last selected index before the current selection
    pub last_selected: Option<usize>,
    /// The first index currently visible in the viewport
    pub window_start: usize,
}

impl ScrollState {
    /// Create a new, uninitialized scroll state
    pub fn new() -> Self {
        Self {
            last_selected: None,
            window_start: 0,
        }
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn selected_scroll_with_direction(
    items: usize,
    max_items: usize,
    selected: Option<usize>,
    state: ScrollState,
) -> (Range<usize>, ScrollState) {
    let mut new_state = state;
    new_state.last_selected = selected;

    let (range, start) =
        selected_scroll_with_anchor(items, max_items, selected, state.window_start);
    new_state.window_start = start;

    (range, new_state)
}

pub(crate) fn selected_scroll_with_anchor(
    items: usize,
    max_items: usize,
    selected: Option<usize>,
    window_start: usize,
) -> (Range<usize>, usize) {
    if max_items == 0 {
        return (0..0, 0);
    }

    if items <= max_items {
        return (0..items, 0);
    }

    let Some(selected) = selected else {
        return (0..max_items.min(items), 0);
    };

    let max_start = items - max_items;
    let mut start = window_start.min(max_start);
    let end = start + max_items;

    // Keep the selected row visible while preserving viewport continuity.
    if selected < start {
        start = selected;
    } else if selected >= end {
        start = selected + 1 - max_items;
    }

    start = start.min(max_start);
    (start..(start + max_items), start)
}

pub type CowStr = Cow<'static, str>;

#[inline]
pub fn strength_bars(s: f32) -> &'static str {
    if s >= 75.0 {
        "▮▮▮▮"
    } else if s >= 55.0 {
        "▮▮▮▯"
    } else if s >= 35.0 {
        "▮▮▯▯"
    } else if s >= 15.0 {
        "▮▯▯▯"
    } else {
        "▯▯▯▯"
    }
}

#[inline]
pub fn humanize_duration(d: Duration) -> String {
    timeago::Formatter::new().convert(d)
}

#[inline]
pub fn strength_color(s: f32) -> Color {
    if s >= 55.0 {
        PATINA.live
    } else if s >= 35.0 {
        PATINA.warn
    } else {
        PATINA.danger
    }
}

const BRAILLE_BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

pub struct BrailleSparkline<'a> {
    data: &'a [usize],
    style: Style,
    max: Option<usize>,
}

impl<'a> BrailleSparkline<'a> {
    pub fn new(data: &'a [usize]) -> Self {
        Self {
            data,
            style: Style::new(),
            max: None,
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    #[allow(dead_code)]
    pub fn max(mut self, max: usize) -> Self {
        self.max = Some(max);
        self
    }
}

impl Widget for BrailleSparkline<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        buf.set_style(area, self.style);

        if area.width == 0 || area.height == 0 || self.data.is_empty() {
            return;
        }

        let n = area.width as usize;
        let m = area.height as usize;
        let sub_cols = 2 * n;
        let sub_rows = 4 * m;

        let slice = if self.data.len() > sub_cols {
            &self.data[self.data.len() - sub_cols..]
        } else {
            self.data
        };

        let max = self
            .max
            .unwrap_or_else(|| slice.iter().copied().max().unwrap_or(0));
        if max == 0 {
            return;
        }

        let mut masks = vec![0u8; n * m];

        let max_f = max as f64;
        let top_bin = (sub_rows - 1) as f64;

        for (sx, &v) in slice.iter().enumerate() {
            let raw = (v as f64 / max_f * top_bin).round();
            let bin = if raw.is_nan() || raw < 0.0 {
                0
            } else if raw > top_bin {
                sub_rows - 1
            } else {
                raw as usize
            };
            let sy = (sub_rows - 1) - bin;

            let cell_col = sx / 2;
            let cell_row = sy / 4;
            let local_col = sx % 2;
            let local_row = sy % 4;

            masks[cell_row * n + cell_col] |= BRAILLE_BITS[local_row][local_col];
        }

        for cell_row in 0..m {
            for cell_col in 0..n {
                let mask = masks[cell_row * n + cell_col];
                if mask == 0 {
                    continue;
                }
                let glyph = char::from_u32(0x2800 | mask as u32).unwrap();
                let x = area.x + cell_col as u16;
                let y = area.y + cell_row as u16;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(glyph).set_style(self.style);
                }
            }
        }
    }
}

pub struct WidgetList<T> {
    children: Vec<T>,
    layout: Layout,
}

impl<T: Widget> WidgetList<T> {
    pub fn new(layout: Layout, children: impl IntoIterator<Item = T>) -> Self {
        Self {
            layout,
            children: children.into_iter().collect(),
        }
    }
}

impl<T: Widget> Widget for WidgetList<T> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let areas = self.layout.split(area);

        self.children
            .into_iter()
            .zip(areas.iter().cloned())
            .for_each(|(w, a)| {
                w.render(a, buf);
            });
    }
}

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L: Widget, R: Widget> Widget for Either<L, R> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        match self {
            Either::Left(l) => l.render(area, buf),
            Either::Right(r) => r.render(area, buf),
        }
    }
}

pub struct Separator {
    pub char: char,
    pub style: Style,
}

impl Separator {
    pub const fn new(c: char) -> Self {
        Self {
            char: c,
            style: Style::new(),
        }
    }

    pub const fn styled(self, style: Style) -> Self {
        Self {
            char: self.char,
            style,
        }
    }
}

impl Widget for Separator {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let text: String = std::iter::repeat_n(self.char, area.width as usize).collect();

        let text = span![self.style; text];

        text.render(area, buf);
    }
}
