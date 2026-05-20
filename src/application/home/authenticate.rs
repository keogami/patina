use std::sync::Arc;

use poppingboba::help::{HelpTable, HelpWidget};
use ratatui::{
    Frame,
    crossterm::event::KeyModifiers,
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

enum InputField {
    Password { visible: bool },
    Checkbox,
}

impl InputField {
    fn is_password(&self) -> bool {
        matches!(self, InputField::Password { .. })
    }

    fn is_checkbox(&self) -> bool {
        matches!(self, InputField::Checkbox)
    }
}

pub struct Authenticate {
    access_point: Arc<AvailableAPDetails>,
    password: tiny_str::TinyString<32>,
    autoconnect: bool,
    selected_field: InputField,
}

impl Authenticate {
    pub fn new(access_point: Arc<AvailableAPDetails>) -> Self {
        Self {
            access_point,
            password: "".into(),
            autoconnect: true,
            selected_field: InputField::Password { visible: false },
        }
    }
}

impl Component for Authenticate {
    fn update(&mut self, _ctx: &RichContext, ev: Message) -> Bubble {
        match ev {
            Message::Crossterm(ratatui::crossterm::event::Event::Key(event))
                if event.code.is_esc() =>
            {
                return Bubble::Yes(Message::FinishAuthentication);
            }
            Message::Crossterm(ratatui::crossterm::event::Event::Key(event))
                if event.code.is_enter() =>
            {
                // TODO actually do the connect before finishing
                // - add a spinner to show that something is happening in the background
                return Bubble::Yes(Message::FinishAuthentication);
            }
            Message::Crossterm(ratatui::crossterm::event::Event::Key(event))
                if event.code.is_backspace() && self.selected_field.is_password() =>
            {
                self.password.pop();
            }
            Message::Crossterm(ratatui::crossterm::event::Event::Key(event))
                if event.modifiers.contains(KeyModifiers::CONTROL)
                    && event.code.is_char('r')
                    && self.selected_field.is_password() =>
            {
                if let InputField::Password { visible } = &mut self.selected_field {
                    *visible = !*visible;
                }
            }
            Message::Crossterm(ratatui::crossterm::event::Event::Key(event))
                if event.code.is_tab() =>
            {
                if self.selected_field.is_password() {
                    self.selected_field = InputField::Checkbox
                } else {
                    self.selected_field = InputField::Password { visible: false }
                }
            }
            Message::Crossterm(ratatui::crossterm::event::Event::Key(event)) => {
                match self.selected_field {
                    InputField::Password { .. } => {
                        let Some(char) = event.code.as_char() else {
                            return Bubble::No;
                        };

                        self.password.push(char);
                    }
                    InputField::Checkbox => {
                        if event.code.is_char(' ') {
                            self.autoconnect = !self.autoconnect;
                        }
                    }
                }
            }
            _ => {}
        }
        Bubble::No
    }

    fn draw(&mut self, _ctx: &RichContext, frame: &mut Frame<'_>) {
        let overlay = Overlay::new()
            .backdrop(Backdrop::new(PATINA.bg_alt))
            .width(constraint!(== 50 %))
            .height(constraint!(== 16))
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
            check_top,
            check_mid,
            check_bot,
            _,
            sep_area,
            _,
            footer_area,
        ] = Layout::vertical(constraints![
            == 1, == 1, == 1, == 1, == 1, == 1, == 1, == 1, == 1, == 1, == 1, *= 2, == 1, == 1, == 1
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

        // Password box: half-block top, filled middle, half-block bottom — only when focused
        let pw_selected = self.selected_field.is_password();
        let edge_style = Style::new().fg(PATINA.bg_alt).bg(PATINA.bg);
        if pw_selected {
            Separator::new('\u{2584}')
                .styled(edge_style)
                .render(pw_top, frame.buffer_mut());
            Block::new()
                .style(Style::new().bg(PATINA.bg_alt))
                .render(pw_mid, frame.buffer_mut());
            Separator::new('\u{2580}')
                .styled(edge_style)
                .render(pw_bot, frame.buffer_mut());
        }

        let pw_bg = if pw_selected {
            PATINA.bg_alt
        } else {
            PATINA.bg
        };
        let pw_inner = pw_mid.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 0,
        });
        let prompt = span!(Style::new().fg(PATINA.accent).bg(pw_bg); "> ");
        let reveal = matches!(self.selected_field, InputField::Password { visible: true });
        let pw_text = if self.password.is_empty() {
            span!(Style::new().fg(PATINA.dim).bg(pw_bg); "min. 8 characters")
        } else if reveal {
            span!(Style::new().fg(PATINA.fg).bg(pw_bg); self.password.as_str())
        } else {
            let masked: String =
                std::iter::repeat_n('\u{2022}', self.password.chars().count()).collect();
            span!(Style::new().fg(PATINA.fg).bg(pw_bg); masked)
        };
        line![prompt, pw_text]
            .style(Style::new().bg(pw_bg))
            .render(pw_inner, frame.buffer_mut());

        // Checkbox row — same 3-row half-block highlight as available list items, only when focused
        let cb_selected = self.selected_field.is_checkbox();
        if cb_selected {
            Separator::new('\u{2584}')
                .styled(edge_style)
                .render(check_top, frame.buffer_mut());
            Block::new()
                .style(Style::new().bg(PATINA.bg_alt))
                .render(check_mid, frame.buffer_mut());
            Separator::new('\u{2580}')
                .styled(edge_style)
                .render(check_bot, frame.buffer_mut());
        }
        let cb_inner = check_mid.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 0,
        });
        let cb_bg = if cb_selected {
            PATINA.bg_alt
        } else {
            PATINA.bg
        };
        let row_style = Style::new().bg(cb_bg);
        let (cb, cb_style) = if self.autoconnect {
            ("[x] ", Style::new().fg(PATINA.accent).bg(cb_bg))
        } else {
            ("[ ] ", Style::new().fg(PATINA.mute).bg(cb_bg))
        };
        let cb_span = span!(cb_style; cb);
        let label = span!(Style::new().fg(PATINA.fg).bg(cb_bg); "Connect automatically");
        let cb_left = line![cb_span, label].left_aligned();
        let cb_right = line![span!(Style::new().fg(PATINA.dim).bg(cb_bg); "autoconnect on save")]
            .right_aligned();
        cb_right
            .style(row_style)
            .render(cb_inner, frame.buffer_mut());
        cb_left
            .style(row_style)
            .render(cb_inner, frame.buffer_mut());

        // Separator above footer
        Separator::new('\x5F')
            .styled(Style::new().fg(PATINA.mute))
            .render(sep_area, frame.buffer_mut());

        // Footer help — keys shown depend on the selected field
        match self.selected_field {
            InputField::Password { visible } => {
                let reveal_label = if visible { "hide" } else { "reveal" };
                let help = HelpWidget::new(HelpTable::new(
                    [
                        ("connect", ("enter", "connect").into()),
                        ("next", ("tab", "next field").into()),
                        ("reveal", ("ctrl+r", reveal_label).into()),
                        ("cancel", ("esc", "cancel").into()),
                    ],
                    &["connect", "next", "reveal", "cancel"],
                    (&[], 0),
                ));
                help.render(footer_area, frame.buffer_mut());
            }
            InputField::Checkbox => {
                let help = HelpWidget::new(HelpTable::new(
                    [
                        ("connect", ("enter", "connect").into()),
                        ("next", ("tab", "next field").into()),
                        ("toggle", ("space", "toggle").into()),
                        ("cancel", ("esc", "cancel").into()),
                    ],
                    &["connect", "next", "toggle", "cancel"],
                    (&[], 0),
                ));
                help.render(footer_area, frame.buffer_mut());
            }
        }
    }
}
