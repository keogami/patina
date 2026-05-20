use ratatui::{
    layout::Layout,
    macros::{constraint, constraints, line, span},
    style::{Color, Style},
    widgets::{Block, Widget},
};

use crate::application::{
    theme::PATINA,
    utils::{
        CowStr, ScrollState, Separator, WidgetList, selected_scroll_with_direction, strength_bars,
        strength_color,
    },
};

pub const ITEM_HEIGHT: u16 = 3;

// TODO: remove this clone, only needed for testing
#[derive(Clone)]
pub struct AvailableAPDetails {
    pub strength: f32,
    pub ssid: CowStr,
    pub security: CowStr,
    pub frequency: CowStr,
    pub channel: u32,
    pub link_speed: CowStr,
}

pub struct AvailableList<'a> {
    pub items: &'a [AvailableAPDetails],
    pub selected: Option<usize>,
    pub scroll_state: ScrollState,
}

impl Widget for AvailableList<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let max_count = area.height.div_euclid(ITEM_HEIGHT);
        let (range, _) = selected_scroll_with_direction(
            self.items.len(),
            max_count as _,
            self.selected,
            self.scroll_state,
        );
        let len = range.len();
        let start = range.start;

        let items = &self.items[range];
        let items = items.iter().enumerate().map(|(idx, item)| AccessPointItem {
            data: item,
            selected: self.selected.is_some_and(|sel| sel == idx + start),
            disabled: self.selected.is_none(),
        });

        let list = WidgetList::new(
            Layout::vertical(std::iter::repeat_n(ITEM_HEIGHT, len).map(|i| constraint!(== i))),
            items,
        );

        list.render(area, buf);
    }
}

pub struct AccessPointItem<'a> {
    pub data: &'a AvailableAPDetails,
    pub selected: bool,
    pub disabled: bool,
}

impl Widget for AccessPointItem<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        debug_assert!(
            area.height >= 3,
            "AccessPointItem must have at least height of 3points, instead had {}",
            area.height
        );

        let disabled_color = |color: Color| {
            if self.disabled { PATINA.mute } else { color }
        };

        if self.selected {
            let edge_style = Style::new().fg(PATINA.bg_alt).bg(PATINA.bg);

            let [top, middle, bottom] =
                Layout::vertical(constraints![== 1, == 1, == 1]).areas(area);

            Separator::new('\u{2584}')
                .styled(edge_style)
                .render(top, buf);
            Block::new()
                .style(Style::new().bg(PATINA.bg_alt))
                .render(middle, buf);
            Separator::new('\u{2580}')
                .styled(edge_style)
                .render(bottom, buf);
        }

        let [_, area] = Layout::vertical(constraints![== 1, == 1]).areas(area);

        let AvailableAPDetails {
            strength,
            ssid,
            security,
            frequency,
            channel,
            link_speed,
        } = self.data;

        let bars =
            span!(disabled_color(strength_color(*strength)); "{}  ", strength_bars(*strength));
        let pct = span!(disabled_color(PATINA.dim); "{}%  ", strength.round() as u32);
        let pad = span!("    ");
        let name = span!(disabled_color(PATINA.fg);"{}  ", ssid);
        let sec = span!(disabled_color(PATINA.dim); security);

        let left = line![pad, bars, pct, name, sec].left_aligned();

        let right =
            line![span!(disabled_color(PATINA.mute); "{frequency} · ch {channel} · {link_speed}")]
                .right_aligned();

        let style = if self.selected {
            Style::new().bg(disabled_color(PATINA.bg_alt))
        } else {
            Style::new()
        };

        right.style(style).render(area, buf);
        left.style(style).render(area, buf);

        if self.selected
            && let Some(cell) = buf.cell_mut((area.x + 1, area.y))
        {
            cell.set_char('\u{3009}')
                .set_style(Style::new().fg(PATINA.accent).bg(PATINA.bg_alt));
        }
    }
}
