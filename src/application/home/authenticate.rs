use std::sync::Arc;

use poppingboba::help::{HelpTable, HelpWidget};
use ratatui::{
    Frame,
    layout::Layout,
    macros::{constraint, constraints, line, span, vertical},
    style::Style,
    widgets::{Block, Widget},
};
use tui_overlay::{Backdrop, Overlay, OverlayState};

use crate::application::{
    component::{Bubble, Component, Message, RichContext},
    home::available::AvailableAPDetails,
    theme::PATINA,
    utils::{Separator, strength_bars, strength_color},
};

pub struct Authenticate {
    pub access_point: Arc<AvailableAPDetails>,
    pub password: String,
    pub autoconnect: bool,
}

impl Component for Authenticate {
    fn update(&mut self, _ctx: &RichContext, _ev: Message) -> Bubble {
        Bubble::No
    }

    fn draw(&mut self, _ctx: &RichContext, frame: &mut Frame<'_>) {
        let overlay = Overlay::new()
            .backdrop(Backdrop::new(PATINA.bg_alt))
            .width(constraint!(== 50 %))
            .height(constraint!(== 14))
            .anchor(tui_overlay::Anchor::Center);

        let mut overlay_state = OverlayState::default();
        overlay_state.open();

        let [overlay_area, _] = vertical![*= 1, == 2].areas(frame.area());

        frame.render_stateful_widget(overlay, overlay_area, &mut overlay_state);

        let overlay_area = overlay_state
            .inner_area()
            .expect("overlay to be open as it is opened explicitly");

        let overlay_area = overlay_area.inner(ratatui::layout::Margin {
            horizontal: 2,
            vertical: 1,
        });

        let AvailableAPDetails {
            strength,
            ssid,
            security,
            frequency,
            channel,
            ..
        } = &*self.access_point;

        let [
            title_area,
            ssid_area,
            _,
            pw_label_area,
            pw_top,
            pw_mid,
            pw_bot,
            _,
            check_area,
            _,
            sep_area,
            _,
            footer_area,
        ] = Layout::vertical(constraints![
            == 1, == 1, == 1, == 1, == 1, == 1, == 1, == 1, == 1, *= 2, == 1, == 1, == 1
        ])
        .areas(overlay_area);

        // Title row: AUTHENTICATE · ▮▮▮▮ 66%
        let title = span!(PATINA.accent; "AUTHENTICATE");
        let dot = span!(PATINA.dim; "  ·  ");
        let bars = span!(strength_color(*strength); "{}", strength_bars(*strength));
        let pct = span!(PATINA.dim; " {}%", strength.round() as u32);
        line![title, dot, bars, pct].render(title_area, frame.buffer_mut());

        // SSID row
        let ssid_span = span!(PATINA.fg; "{}  ", ssid);
        let sec_span = span!(PATINA.dim; "{}", security);
        let left = line![ssid_span, sec_span].left_aligned();
        let right = line![span!(PATINA.mute; "{frequency} · ch {channel}")].right_aligned();
        right.render(ssid_area, frame.buffer_mut());
        left.render(ssid_area, frame.buffer_mut());

        // PASSWORD label
        line![span!(PATINA.dim; "PASSWORD")].render(pw_label_area, frame.buffer_mut());

        // Password box: half-block top, filled middle, half-block bottom
        let edge_style = Style::new().fg(PATINA.bg_alt).bg(PATINA.bg);
        Separator::new('\u{2584}')
            .styled(edge_style)
            .render(pw_top, frame.buffer_mut());
        Block::new()
            .style(Style::new().bg(PATINA.bg_alt))
            .render(pw_mid, frame.buffer_mut());
        Separator::new('\u{2580}')
            .styled(edge_style)
            .render(pw_bot, frame.buffer_mut());

        let pw_inner = pw_mid.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 0,
        });
        let prompt = span!(Style::new().fg(PATINA.accent).bg(PATINA.bg_alt); "> ");
        let pw_text = if self.password.is_empty() {
            span!(Style::new().fg(PATINA.dim).bg(PATINA.bg_alt); "min. 8 characters")
        } else {
            let masked: String =
                std::iter::repeat_n('\u{2022}', self.password.chars().count()).collect();
            span!(Style::new().fg(PATINA.fg).bg(PATINA.bg_alt); masked)
        };
        line![prompt, pw_text]
            .style(Style::new().bg(PATINA.bg_alt))
            .render(pw_inner, frame.buffer_mut());

        // Checkbox row
        let (cb, cb_style) = if self.autoconnect {
            ("[x] ", Style::new().fg(PATINA.accent))
        } else {
            ("[ ] ", Style::new().fg(PATINA.mute))
        };
        let cb_span = span!(cb_style; cb);
        let label = span!(PATINA.fg; "Connect automatically");
        let cb_left = line![cb_span, label].left_aligned();
        let cb_right = line![span!(PATINA.dim; "autoconnect on save")].right_aligned();
        cb_right.render(check_area, frame.buffer_mut());
        cb_left.render(check_area, frame.buffer_mut());

        // Separator above footer
        Separator::new('\x5F')
            .styled(Style::new().fg(PATINA.mute))
            .render(sep_area, frame.buffer_mut());

        // Footer help
        let help = HelpWidget::new(HelpTable::new(
            [
                ("connect", ("enter", "connect").into()),
                ("next", ("tab", "next field").into()),
                ("toggle", ("space", "toggle").into()),
                ("reveal", ("^r", "reveal").into()),
                ("cancel", ("esc", "cancel").into()),
            ],
            &["connect", "next", "toggle", "reveal", "cancel"],
            (&[], 0),
        ));
        help.render(footer_area, frame.buffer_mut());
    }
}
