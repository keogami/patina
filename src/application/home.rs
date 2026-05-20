use std::{cell::Cell, sync::Arc, thread::spawn, time::Duration};

use circular_buffer::CircularBuffer;
use poppingboba::{
    help::{HelpTable, HelpWidget},
    spinner::{Spinner, SpinnerType},
};
use ratatui::{
    layout::Layout,
    macros::{constraint, constraints, line, span, text, vertical},
    style::Color,
    widgets::Widget,
};

use crate::application::{
    component::{Available, Bubble, Connected, RichContext},
    home::{
        available::{AvailableAPDetails, AvailableList, ITEM_HEIGHT as AP_ITEM_HEIGHT},
        connected::{ConnectedList, ConnectionData, ITEM_HEIGHT},
    },
    theme::PATINA,
    utils::{
        CowStr,
        Either::{Left, Right},
        ScrollState, Separator, WidgetList, selected_scroll_with_direction,
    },
};

pub mod authenticate;
pub mod available;
pub mod connected;

use super::component::{Component, Message};

pub struct HomeData {
    loading: Option<Spinner>,
    connected: Connected,
    available: Available,
    selected: Selected,
    connected_scroll_state: ScrollState,
    available_scroll_state: ScrollState,
    available_max_items: Cell<usize>,
    authenticate: Option<authenticate::Authenticate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneType {
    Connected,
    Available,
}

struct ConnectionsHeader {
    pub saved_count: usize,
    pub active_count: usize,
    pub selected: Option<(usize, usize)>, // (idx, total)
    pub disabled: bool,
}

struct Header {
    pub hostname: CowStr,
    // TODO: make a status enum based on nm connectivity status, update real-time
    pub online: bool,
    pub active_count: u64,
    pub nm_version: CowStr,
}

impl Widget for &ConnectionsHeader {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let disabled_color = |color: Color| {
            if self.disabled { PATINA.mute } else { color }
        };

        let title = span!(disabled_color(PATINA.accent); "CONNECTIONS  ");
        let status = span!(disabled_color(PATINA.mute); "{} saved · {} active", self.saved_count, self.active_count);

        let line = line![title, status];

        if let Some((idx, total)) = self.selected {
            let tag = span!(disabled_color(PATINA.mute); "selected ");
            let page = span!(disabled_color(PATINA.dim); "{}", idx + 1);
            let total = span!(disabled_color(PATINA.mute); "/{total}");

            let line = line![tag, page, total].right_aligned();
            line.render(area, buf);
        }

        line.render(area, buf);
    }
}

impl Widget for &Header {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let patina = span!("patina");
        let diamond = span!(PATINA.accent; "  ◆  ");
        let hostname = span!(PATINA.soft; self.hostname);
        let dot = span!(PATINA.dim; "  ·  ");

        let status = span!(PATINA.live; "{} · {} active", if self.online { "online" }  else {"offline"}, self.active_count);

        let left = line![patina, diamond, hostname, dot, status].left_aligned();

        let version = span!(PATINA.dim; self.nm_version);
        let right = line![version].right_aligned();

        // TODO: handle the case where there's not enough room, and truncation is needed
        right.render(area, buf);
        left.render(area, buf);
    }
}

enum Help {
    Connected,
    Available,
}

impl Widget for Help {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        match self {
            Help::Connected => {
                let w = HelpWidget::new(HelpTable::new(
                    [
                        ("quit", ("q", "quit").into()),
                        ("nav", ("j/k", "navigate").into()),
                        ("con", ("enter", "disconnect").into()),
                        ("tab", ("tab", "move to access points").into()),
                        ("scan", ("s", "force scan").into()),
                    ],
                    &["nav", "con", "scan", "quit"],
                    (&[], 0),
                ));

                w.render(area, buf);
            }
            Help::Available => {
                let w = HelpWidget::new(HelpTable::new(
                    [
                        ("quit", ("q", "quit").into()),
                        ("nav", ("j/k", "navigate").into()),
                        ("con", ("enter", "connect").into()),
                        ("tab", ("tab", "move to connections").into()),
                        ("scan", ("s", "force scan").into()),
                    ],
                    &["nav", "con", "scan", "quit"],
                    (&[], 0),
                ));

                w.render(area, buf);
            }
        }
    }
}

struct AccessPointsHeader {
    pub in_range: u64,
    pub scanned_ago: CowStr,
    pub selected: Option<(usize, usize)>, // (idx, total)
    pub disabled: bool,
}

impl Widget for &AccessPointsHeader {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let disabled_color = |color: Color| {
            if self.disabled { PATINA.mute } else { color }
        };

        let title = span!(disabled_color(PATINA.accent); "ACCESS POINTS  ");
        let status = span!(disabled_color(PATINA.mute); "{} in range · scanned {}", self.in_range, self.scanned_ago);

        let line = line![title, status];

        if let Some((idx, total)) = self.selected {
            let tag = span!(disabled_color(PATINA.mute); "selected ");
            let page = span!(disabled_color(PATINA.dim); "{}", idx + 1);
            let total = span!(disabled_color(PATINA.mute); "/{total}");

            let line = line![tag, page, total].right_aligned();
            line.render(area, buf);
        }

        line.render(area, buf);
    }
}

enum Selected {
    Connected(usize),
    Available(usize),
    None,
}

impl Selected {
    fn down(&self, con_len: usize, avail_len: usize) -> Self {
        match self {
            Selected::Connected(idx) => {
                let next = idx + 1;
                if next == con_len {
                    Self::Available(0)
                } else {
                    Self::Connected(next)
                }
            }
            Selected::Available(idx) => {
                let next = idx + 1;
                if next == avail_len {
                    Self::Available(*idx)
                } else {
                    Self::Available(next)
                }
            }
            Selected::None => {
                if con_len > 0 {
                    Self::Connected(0)
                } else if avail_len > 0 {
                    Self::Available(0)
                } else {
                    Self::None
                }
            }
        }
    }

    fn up(&self, con_len: usize, _avail_len: usize) -> Self {
        match self {
            Selected::Connected(idx) => {
                if *idx == 0 {
                    Self::None
                } else {
                    Self::Connected(idx - 1)
                }
            }
            Selected::Available(idx) => {
                let idx = *idx;
                if idx == 0 && con_len != 0 {
                    Self::Connected(con_len - 1)
                } else if idx == 0 && con_len == 0 {
                    Self::None
                } else {
                    Self::Available(idx - 1)
                }
            }
            Selected::None => Self::None,
        }
    }

    fn tab(&self, con_len: usize, avail_len: usize) -> Self {
        match self {
            Selected::Connected(idx) => {
                if avail_len == 0 {
                    Self::Connected(*idx)
                } else {
                    Self::Available(0)
                }
            }
            Selected::Available(idx) => {
                if con_len == 0 {
                    Self::Available(*idx)
                } else {
                    Self::Connected(0)
                }
            }
            Selected::None => {
                if con_len != 0 {
                    Self::Connected(0)
                } else if avail_len != 0 {
                    Self::Available(0)
                } else {
                    Self::None
                }
            }
        }
    }

    fn connected_selected(&self) -> Option<usize> {
        match self {
            Selected::Connected(idx) => Some(*idx),
            _ => None,
        }
    }
    fn available_selected(&self) -> Option<usize> {
        match self {
            Selected::Available(idx) => Some(*idx),
            _ => None,
        }
    }
}

impl HomeData {
    pub fn new(ctx: &RichContext) -> Self {
        let loading = Spinner::new(SpinnerType::dot(), ctx.fps);
        let connected = Connected { list: Vec::new() };
        let available = Available { list: Vec::new() };
        let selected = Selected::None;

        // TODO: right now, when the app starts, no section is selected. change
        // that to select the very first item in the connection list. of course,
        // handling the edge cases
        spawn({
            let message_tx = ctx.message.clone();
            move || {
                // mock loading, uncomment to checkout loading state
                // std::thread::sleep(std::time::Duration::from_secs(5));
                let available: Vec<_> = [
                    AvailableAPDetails {
                        strength: 82.,
                        ssid: "Overcast-5G".into(),
                        security: "WPA2".into(),
                        frequency: "5.22 GHz".into(),
                        channel: 44,
                        link_speed: "650 Mbps".into(),
                    },
                    AvailableAPDetails {
                        strength: 71.,
                        ssid: "Acme-Corp".into(),
                        security: "WPA2-E".into(),
                        frequency: "5.75 GHz".into(),
                        channel: 149,
                        link_speed: "867 Mbps".into(),
                    },
                    AvailableAPDetails {
                        strength: 58.,
                        ssid: "FiberLink_9A82".into(),
                        security: "WPA3".into(),
                        frequency: "6.13 GHz".into(),
                        channel: 37,
                        link_speed: "1201 Mbps".into(),
                    },
                    AvailableAPDetails {
                        strength: 42.,
                        ssid: "xfinitywifi".into(),
                        security: "Open".into(),
                        frequency: "2.41 GHz".into(),
                        channel: 1,
                        link_speed: "150 Mbps".into(),
                    },
                    AvailableAPDetails {
                        strength: 28.,
                        ssid: "TP-Link_5544".into(),
                        security: "WPA2".into(),
                        frequency: "2.44 GHz".into(),
                        channel: 6,
                        link_speed: "300 Mbps".into(),
                    },
                ]
                .into_iter()
                .cycle()
                .take(24)
                .map(Arc::new)
                .collect();

                let conns: Vec<_> = [
                    ConnectionData::WifiActive {
                        strength: 82.,
                        name: "Overcast-5G".into(),
                        interface: "wlan0".into(),
                        ip: "10.0.0.147/24".into(),
                        frequency: "5 GHz".into(),
                        link_speed: "650 Mbps".into(),
                        versions: "v4+v6".into(),
                        throughput: CircularBuffer::from_iter(
                            [0, 5, 9, 14, 11, 8, 2, 13].repeat(8),
                        ),
                    },
                    ConnectionData::WiredActive {
                        interface: "eth0".into(),
                        ip: "192.168.4.22/24".into(),
                        name: "eth0".into(),
                        versions: "v4+v6".into(),
                        throughput: CircularBuffer::from_iter(std::iter::repeat_n(0, 16)),
                    },
                    ConnectionData::WifiInactive {
                        name: "Acme-Corp".into(),
                        last_used: Duration::from_secs(60 * 60 * 24),
                        autoconnect: true,
                        metered: false,
                        tag: "WPA2-Enterprise".into(),
                    },
                    ConnectionData::WifiInactive {
                        name: "Pixel-Tether".into(),
                        last_used: Duration::from_secs(60 * 60 * 24 * 14),
                        autoconnect: false,
                        metered: true,
                        tag: "WPA3".into(),
                    },
                ]
                .into_iter()
                .cycle()
                .take(12)
                .map(Arc::new)
                .collect();

                let available = Available { list: available };
                let connected = Connected { list: conns };
                message_tx.send(Message::LoadConnected(connected)).unwrap();
                message_tx.send(Message::LoadAvailable(available)).unwrap();
                message_tx.send(Message::FinishLoading).unwrap();
            }
        });

        Self {
            loading: Some(loading),
            connected,
            available,
            selected,
            connected_scroll_state: ScrollState::new(),
            available_scroll_state: ScrollState::new(),
            available_max_items: Cell::new(ctx.connection_maxitem),
            authenticate: None,
        }
    }

    fn sync_scroll_states(&mut self, connected_max_items: usize) {
        match self.selected {
            Selected::Connected(_) => {
                let (_, connected_state) = selected_scroll_with_direction(
                    self.connected.list.len(),
                    connected_max_items,
                    self.selected.connected_selected(),
                    self.connected_scroll_state,
                );
                self.connected_scroll_state = connected_state;
            }
            Selected::Available(_) => {
                let available_max_items = self.available_max_items.get().max(1);

                let (_, available_state) = selected_scroll_with_direction(
                    self.available.list.len(),
                    available_max_items,
                    self.selected.available_selected(),
                    self.available_scroll_state,
                );
                self.available_scroll_state = available_state;
            }
            Selected::None => {}
        }
    }

    fn handle_tab_press(&mut self, max_items: usize) {
        let con_len = self.connected.list.len();
        let avail_len = self.available.list.len();

        // Get current pane before switch
        let current_pane = if self.selected.connected_selected().is_some() {
            Some(PaneType::Connected)
        } else if self.selected.available_selected().is_some() {
            Some(PaneType::Available)
        } else {
            None
        };

        // Perform the tab switch
        self.selected = self.selected.tab(con_len, avail_len);

        // Get new pane after switch
        let new_pane = if self.selected.connected_selected().is_some() {
            Some(PaneType::Connected)
        } else if self.selected.available_selected().is_some() {
            Some(PaneType::Available)
        } else {
            None
        };

        // If we switched panes, reset the scroll state of the new pane
        if current_pane != new_pane {
            match new_pane {
                Some(PaneType::Connected) => {
                    self.connected_scroll_state = ScrollState::new();
                }
                Some(PaneType::Available) => {
                    self.available_scroll_state = ScrollState::new();
                }
                None => {}
            }
        }

        self.sync_scroll_states(max_items);
    }
}

/// Checks whether an AP requires authentication before we can connect
fn needs_authentication(_access_point: &AvailableAPDetails) -> anyhow::Result<bool> {
    Ok(true)
}

impl Component for HomeData {
    fn update(&mut self, ctx: &RichContext, mut ev: Message) -> Bubble {
        if let Some(ref mut auth) = self.authenticate {
            let bubble = auth.update(ctx, ev);
            match bubble {
                Bubble::Yes(message) => ev = message,
                Bubble::No => return Bubble::No,
            }
        }

        match ev {
            Message::Crossterm(ev) => {
                match ev {
                    ratatui::crossterm::event::Event::Key(key_event)
                        if key_event.kind != ratatui::crossterm::event::KeyEventKind::Press =>
                    {
                        // Ignore key release/repeat events to avoid double-processing.
                    }
                    ratatui::crossterm::event::Event::Key(key_event)
                        if key_event.code.is_char('j') =>
                    {
                        let con_len = self.connected.list.len();
                        let avail_len = self.available.list.len();
                        self.selected = self.selected.down(con_len, avail_len);
                        self.sync_scroll_states(ctx.connection_maxitem);
                    }
                    ratatui::crossterm::event::Event::Key(key_event)
                        if key_event.code.is_char('k') =>
                    {
                        let con_len = self.connected.list.len();
                        let avail_len = self.available.list.len();
                        self.selected = self.selected.up(con_len, avail_len);
                        self.sync_scroll_states(ctx.connection_maxitem);
                    }
                    ratatui::crossterm::event::Event::Key(key_event) if key_event.code.is_tab() => {
                        self.handle_tab_press(ctx.connection_maxitem);
                    }
                    ratatui::crossterm::event::Event::Key(key_event)
                        if key_event.code.is_enter() =>
                    {
                        if let Some(s) = self.selected.available_selected()
                            && needs_authentication(&self.available.list[s]).is_ok_and(|n| n)
                        {
                            ctx.message
                                .send(Message::Authenticate(
                                    super::component::AuthenticationData {
                                        access_point: self.available.list[s].clone(),
                                    },
                                ))
                                .unwrap();
                        }
                    }
                    _ => {
                        // noop
                    }
                }
            }
            Message::GlobalTick => {
                if let Some(spinner) = self.loading.as_mut() {
                    spinner.tick();
                }
            }
            Message::LoadConnected(connected) => self.connected = connected,
            Message::LoadAvailable(available) => self.available = available,
            Message::FinishLoading => self.loading = None,
            Message::FinishAuthentication => {
                self.authenticate = None;
            }
            Message::Authenticate(authentication_data) => {
                self.authenticate = Some(authenticate::Authenticate {
                    access_point: authentication_data.access_point,
                    password: "".into(),
                    autoconnect: true,
                });
            }
        }

        Bubble::No
    }

    fn draw(&mut self, ctx: &RichContext, frame: &mut ratatui::Frame<'_>) {
        let spinner = &self.loading;
        if let Some(spinner) = spinner.as_ref() {
            let text = " Loading patina";
            let center = frame
                .area()
                .centered(constraint!(== text.len() as u16 + 1), constraint!(== 1));

            let [spinner_area, text_area] =
                Layout::horizontal(constraints![== 1, *=  text.len() as u16]).areas(center);

            frame.render_widget(spinner, spinner_area);
            frame.render_widget(line![text], text_area);
            return;
        }

        let (header_widget, header_constraint) = (
            Header {
                hostname: "stacyweiss".into(),
                online: true,
                active_count: 2,
                nm_version: "NM 2.23".into(),
            },
            constraint!(== 1),
        );

        let (header_sep_widget, header_sep_constraint) = (
            Separator::new('\x5F').styled(PATINA.mute.into()),
            constraint!(== 2),
        );

        let connected = &self.connected;
        let total_connected = connected.list.len();

        let (connections_header_widget, connections_header_constraint) = (
            ConnectionsHeader {
                saved_count: 5,
                active_count: 2,
                disabled: self.selected.connected_selected().is_none(),
                selected: self
                    .selected
                    .connected_selected()
                    .map(|idx| (idx, total_connected)),
            },
            constraint!(== 1),
        );

        // TODO: when scrolling crosses the pane boundary, the connected list snaps back to rendering from 0
        // keep additional data to avoid that snapping
        let (connected_widget, connected_constraint) = if !connected.list.is_empty() {
            (
                Left(ConnectedList {
                    items: &connected.list,
                    selected: self.selected.connected_selected(),
                    // TODO: load from app context
                    max_items: 5,
                    scroll_state: self.connected_scroll_state,
                }),
                // TODO: load from AppContext
                constraint!(== connected.list.len().min(5) as u16 * ITEM_HEIGHT),
            )
        } else {
            let text = "There are no connections";
            let text = text![line![], line![text].centered(),];

            (Right(text), constraint!(== 3))
        };

        let available = &self.available;
        let (access_points_header_widget, access_points_header_constraint) = (
            AccessPointsHeader {
                in_range: available.list.len() as u64,
                scanned_ago: "just now".into(),
                disabled: self.selected.available_selected().is_none(),
                selected: self
                    .selected
                    .available_selected()
                    .map(|idx| (idx, available.list.len())),
            },
            constraint!(== 1),
        );

        let (available_widget, available_constraint) = if !available.list.is_empty() {
            (
                Left(AvailableList {
                    items: &available.list,
                    selected: self.selected.available_selected(),
                    scroll_state: self.available_scroll_state,
                }),
                constraint!(*= 1),
            )
        } else {
            let text = "There are no access points available";
            let text = text![line![text].centered()];

            (
                Right(WidgetList::new(
                    Layout::vertical(constraints![== 1]).flex(ratatui::layout::Flex::SpaceAround),
                    text,
                )),
                constraint!(*= 1),
            )
        };

        let help = match self.selected {
            Selected::Connected(_) => Help::Connected,
            Selected::Available(_) => Help::Available,
            Selected::None => Help::Connected,
        };

        let margined = frame.area().inner(ratatui::layout::Margin {
            horizontal: 2,
            vertical: 1,
        });

        let [header_area, hs_area, ch_area, c_area, ah_area, a_area, _] = Layout::vertical([
            header_constraint,
            header_sep_constraint,
            connections_header_constraint,
            connected_constraint,
            access_points_header_constraint,
            available_constraint,
            constraint!(== 2),
        ])
        .areas(margined);

        // Persist the currently visible AP viewport size so key handling uses
        // the same window size as rendering on the next update.
        let ap_max = a_area.height.div_euclid(AP_ITEM_HEIGHT) as usize;
        self.available_max_items.set(ap_max.max(1));

        frame.render_widget(&header_widget, header_area);
        frame.render_widget(header_sep_widget, hs_area);
        frame.render_widget(&connections_header_widget, ch_area);
        frame.render_widget(connected_widget, c_area);
        frame.render_widget(&access_points_header_widget, ah_area);
        frame.render_widget(available_widget, a_area);

        let [_, h_area] = vertical![*= 1, == 1].areas(frame.area());

        frame.render_widget(help, h_area);

        if let Some(ref mut auth) = self.authenticate {
            auth.draw(ctx, frame);
        }
    }
}
