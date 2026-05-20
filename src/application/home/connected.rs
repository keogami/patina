use std::time::Duration;

use circular_buffer::CircularBuffer;
use ratatui::{
    layout::{Layout, Rect},
    macros::{constraint, constraints, line, span},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Widget},
};

use crate::application::{
    theme::PATINA,
    utils::{
        BrailleSparkline, CowStr, ScrollState, Separator, WidgetList, humanize_duration,
        selected_scroll_with_direction, strength_bars,
    },
};

const SPARKLINE_WIDTH: u16 = 24;
const MIN_LEFT_WIDTH: u16 = 0;

pub const ITEM_HEIGHT: u16 = 4;

// TODO: remove this clone, only needed for testing
#[derive(Clone)]
pub enum ConnectionData {
    WifiActive {
        strength: f32,
        name: CowStr,
        interface: CowStr,
        ip: CowStr,
        frequency: CowStr,
        link_speed: CowStr,
        versions: CowStr,
        throughput: CircularBuffer<48, usize>,
    },
    WiredActive {
        interface: CowStr,
        ip: CowStr,
        name: CowStr,
        versions: CowStr,
        throughput: CircularBuffer<48, usize>,
    },
    WifiInactive {
        name: CowStr,
        last_used: Duration,
        autoconnect: bool,
        metered: bool,
        tag: CowStr,
    },
}

pub struct ConnectedList<'a> {
    pub items: &'a [ConnectionData],
    pub selected: Option<usize>,
    pub max_items: usize,
    pub scroll_state: ScrollState,
}

impl Widget for ConnectedList<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let (range, _) = selected_scroll_with_direction(
            self.items.len(),
            self.max_items,
            self.selected,
            self.scroll_state,
        );
        let start = range.start;
        let len = range.len();
        let items = self.items[range]
            .iter()
            .enumerate()
            .map(|(idx, item)| ConnectionItem {
                data: item,
                selected: self.selected.is_some_and(|sel| sel == start + idx),
                disabled: self.selected.is_none(),
            });

        let list = WidgetList::new(
            Layout::vertical(std::iter::repeat_n(ITEM_HEIGHT, len).map(|i| constraint!(== i))),
            items,
        );

        list.render(area, buf);
    }
}

pub struct ConnectionItem<'a> {
    pub data: &'a ConnectionData,
    pub selected: bool,
    pub disabled: bool,
}

impl Widget for ConnectionItem<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let disabled_color = |color: Color| {
            if self.disabled { PATINA.mute } else { color }
        };

        let [top_edge, text_top, text_bottom, bot_edge] =
            Layout::vertical(constraints![== 1, == 1, == 1, == 1]).areas(area);

        let text_band = Rect {
            x: area.x,
            y: text_top.y,
            width: area.width,
            height: text_top.height + text_bottom.height,
        };

        if self.selected {
            let edge_style = Style::new().fg(PATINA.bg_alt).bg(PATINA.bg);
            Separator::new('\u{2584}')
                .styled(edge_style)
                .render(top_edge, buf);
            Separator::new('\u{2580}')
                .styled(edge_style)
                .render(bot_edge, buf);

            Block::new()
                .style(Style::new().bg(PATINA.bg_alt))
                .render(text_band, buf);

            if let Some(cell) = buf.cell_mut((area.x + 1, text_top.y)) {
                cell.set_char('\u{3009}')
                    .set_style(Style::new().fg(PATINA.accent).bg(PATINA.bg_alt));
            }
        }

        // essentially a left-only margin of 2 points
        let margin = 4;
        let area = Rect {
            x: text_band.x + margin,
            y: text_band.y,
            width: text_band.width - margin,
            height: text_band.height,
        };

        match self.data {
            ConnectionData::WifiActive {
                strength,
                name,
                interface,
                ip,
                frequency,
                link_speed,
                versions,
                throughput,
            } => {
                let bars = span!(disabled_color(PATINA.live); "{}  ", strength_bars(*strength));
                let strength = span!(disabled_color(PATINA.dim);"{}%  ", strength.round() as u32);
                let name = span!(disabled_color(PATINA.fg);"{}", name);
                let top = line![bars, strength, name];

                let bottom = line![
                    span!(disabled_color(PATINA.mute); "{ip}  "),
                    span!(disabled_color(PATINA.dim); "on "),
                    span!(disabled_color(PATINA.soft); "{interface} "),
                    span!(disabled_color(PATINA.mute); "· {frequency} · {link_speed} · "),
                    span!(disabled_color(PATINA.live); versions)
                ];

                render_top_with_sparkline(area, buf, top, throughput, disabled_color(PATINA.live));
                let [_, bottom_row] = Layout::vertical(constraints![== 1, == 1]).areas(area);
                bottom.render(bottom_row, buf);
            }
            ConnectionData::WiredActive {
                interface,
                ip,
                name,
                versions,
                throughput,
            } => {
                let bars = span!(disabled_color(PATINA.live); "═══  ");
                let name = span!(disabled_color(PATINA.fg); "{} ", name);
                let kind = span!(disabled_color(PATINA.dim); "· Wired");
                let top = line![bars, name, kind];

                let bottom = line![
                    span!(disabled_color(PATINA.mute); "{ip}  "),
                    span!(disabled_color(PATINA.dim); "on "),
                    span!(disabled_color(PATINA.soft); "{interface} "),
                    span!(disabled_color(PATINA.dim); "· "),
                    span!(disabled_color(PATINA.live); versions)
                ];

                render_top_with_sparkline(area, buf, top, throughput, disabled_color(PATINA.live));
                let [_, bottom_row] = Layout::vertical(constraints![== 1, == 1]).areas(area);
                bottom.render(bottom_row, buf);
            }
            ConnectionData::WifiInactive {
                name,
                last_used,
                autoconnect,
                metered,
                tag,
            } => {
                let kind = span!(disabled_color(PATINA.fg_alt); "wifi  ");
                let name = span!(disabled_color(PATINA.fg); "{}", name);
                let top_left = line![kind, name].left_aligned();

                let tag = span!(disabled_color(PATINA.dim); tag);
                let top_right = line![tag].right_aligned();

                let used_label = span!(disabled_color(PATINA.mute); "used ");
                let used_val =
                    span!(disabled_color(PATINA.dim); "{}  ", humanize_duration(*last_used));
                let ac_label = span!(disabled_color(PATINA.mute); "autoconnect ");
                let ac_val = if *autoconnect {
                    span!(disabled_color(PATINA.live); "on")
                } else {
                    span!(disabled_color(PATINA.dim); "off")
                };

                let mut bottom_spans = vec![used_label, used_val, ac_label, ac_val];
                if *metered {
                    bottom_spans.push(span!("  "));
                    bottom_spans.push(span!(disabled_color(PATINA.warn); "metered"));
                }
                let bottom = ratatui::text::Line::from(bottom_spans);

                let [top_area, bottom_area] =
                    Layout::vertical(constraints![== 1, == 1]).areas(area);

                top_right.render(top_area, buf);
                top_left.render(top_area, buf);
                bottom.render(bottom_area, buf);
            }
        }
    }
}

fn render_top_with_sparkline(
    area: Rect,
    buf: &mut ratatui::prelude::Buffer,
    top: Line<'_>,
    throughput: &CircularBuffer<48, usize>,
    spark_color: Color,
) {
    let [top_row, _] = Layout::vertical(constraints![== 1, == 1]).areas(area);

    if top_row.width > MIN_LEFT_WIDTH + SPARKLINE_WIDTH {
        let [left, _gap, right] =
            Layout::horizontal(constraints![*= 1, == 1, == SPARKLINE_WIDTH]).areas(top_row);
        top.render(left, buf);
        let data: Vec<usize> = throughput.iter().copied().collect();
        BrailleSparkline::new(&data)
            .style(Style::new().fg(spark_color))
            .render(right, buf);
    } else {
        top.render(top_row, buf);
    }
}
