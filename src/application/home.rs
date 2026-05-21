use std::{cell::Cell, sync::Arc, time::Duration};

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
        info::{Info, InfoTarget},
    },
    theme::PATINA,
    utils::{
        CowStr,
        Either::{Left, Right},
        ScrollState, Separator, WidgetList, selected_scroll_with_direction,
    },
};
use crate::backend::{bridge::BridgeResult, types::*};

pub mod authenticate;
pub mod available;
pub mod connected;
pub mod info;

use super::component::{Component, Message};

fn map_active_to_connection_data(active: ActiveConnection) -> ConnectionData {
    match active.connection_type {
        ConnectionType::Wifi => ConnectionData::WifiActive {
            strength: 82.0,
            name: active.name.into(),
            interface: active.interface.into(),
            ip: active.ip4_config.as_ref()
                .and_then(|c| c.addresses.first())
                .map(|a| format!("{}/{}", a.address, a.prefix).into())
                .unwrap_or_else(|| "0.0.0.0/0".into()),
            frequency: "5 GHz".into(),
            link_speed: "650 Mbps".into(),
            versions: if active.ip6_config.is_some() { "v4+v6".into() } else { "v4".into() },
            throughput: CircularBuffer::new(),
            bssid: "00:00:00:00:00:00".into(),
            gateway: active.ip4_config.and_then(|c| c.gateway).unwrap_or_default().into(),
            dns: Vec::new(),
            mac: "00:00:00:00:00:00".into(),
            mtu: 1500,
            security: "WPA2".into(),
            channel: 44,
            autoconnect: true,
            metered: false,
            uptime: Duration::from_secs(0),
            rx_bytes: 0,
            tx_bytes: 0,
            signal_history: CircularBuffer::new(),
            rx_throughput: CircularBuffer::new(),
            tx_throughput: CircularBuffer::new(),
        },
        ConnectionType::Ethernet => ConnectionData::WiredActive {
            interface: active.interface.into(),
            ip: active.ip4_config.as_ref()
                .and_then(|c| c.addresses.first())
                .map(|a| format!("{}/{}", a.address, a.prefix).into())
                .unwrap_or_else(|| "0.0.0.0/0".into()),
            name: active.name.into(),
            versions: if active.ip6_config.is_some() { "v4+v6".into() } else { "v4".into() },
            throughput: CircularBuffer::new(),
            gateway: active.ip4_config.and_then(|c| c.gateway).unwrap_or_default().into(),
            dns: Vec::new(),
            mac: "00:00:00:00:00:00".into(),
            mtu: 1500,
            link_speed: "1 Gbps".into(),
            autoconnect: true,
            uptime: Duration::from_secs(0),
            rx_bytes: 0,
            tx_bytes: 0,
            rx_throughput: CircularBuffer::new(),
            tx_throughput: CircularBuffer::new(),
        },
        _ => ConnectionData::WiredActive {
            interface: active.interface.into(),
            ip: "0.0.0.0/0".into(),
            name: active.name.into(),
            versions: "v4".into(),
            throughput: CircularBuffer::new(),
            gateway: "".into(),
            dns: Vec::new(),
            mac: "00:00:00:00:00:00".into(),
            mtu: 1500,
            link_speed: "".into(),
            autoconnect: true,
            uptime: Duration::from_secs(0),
            rx_bytes: 0,
            tx_bytes: 0,
            rx_throughput: CircularBuffer::new(),
            tx_throughput: CircularBuffer::new(),
        },
    }
}

fn map_ap_to_available_details(ap: AccessPoint) -> AvailableAPDetails {
    AvailableAPDetails {
        strength: ap.strength as f32,
        ssid: String::from_utf8_lossy(&ap.ssid).into_owned().into(),
        security: match ap.security {
            SecurityType::Open => "Open".into(),
            SecurityType::Wep => "WEP".into(),
            SecurityType::WpaPsk { version } => match version {
                WpaVersion::Wpa1 => "WPA".into(),
                WpaVersion::Wpa2 => "WPA2".into(),
                WpaVersion::Wpa3 => "WPA3".into(),
            },
            SecurityType::WpaEnterprise { version } => match version {
                WpaVersion::Wpa1 => "WPA-E".into(),
                WpaVersion::Wpa2 => "WPA2-E".into(),
                WpaVersion::Wpa3 => "WPA3-E".into(),
            },
        },
        frequency: format!("{} GHz", ap.frequency as f32 / 1000.0).into(),
        channel: ap.channel,
        link_speed: format!("{} Mbps", ap.max_bitrate / 1000).into(),
        bssid: ap.bssid.into(),
        saved: false,
        signal_history: CircularBuffer::new(),
    }
}

fn map_profile_to_inactive(profile: ConnectionProfile) -> ConnectionData {
    use std::time::Duration;
    match profile.connection_type {
        ConnectionType::Wifi => ConnectionData::WifiInactive {
            name: profile.name.into(),
            last_used: Duration::from_secs(0), // never used
            autoconnect: profile.autoconnect,
            metered: false, // TODO: extract from profile settings if available
            tag: "saved".into(),
            security: "WPA2".into(), // TODO: extract from profile settings
            bssid: None, // No specific BSSID for saved profiles
            saved_ip_method: "auto".into(), // TODO: extract from profile settings
            saved_dns: vec![], // TODO: extract from profile settings
        },
        // There's no WiredInactive variant, use WiredActive as placeholder
        _ => ConnectionData::WiredActive {
            interface: profile.interface_name.unwrap_or_default().into(),
            ip: "0.0.0.0/0".into(),
            name: profile.name.into(),
            versions: "v4".into(),
            throughput: circular_buffer::CircularBuffer::new(),
            gateway: "".into(),
            dns: vec![],
            mac: "00:00:00:00:00:00".into(),
            mtu: 1500,
            link_speed: "".into(),
            autoconnect: profile.autoconnect,
            uptime: Duration::from_secs(0),
            rx_bytes: 0,
            tx_bytes: 0,
            rx_throughput: circular_buffer::CircularBuffer::new(),
            tx_throughput: circular_buffer::CircularBuffer::new(),
        },
    }
}

pub struct HomeData {
    loading: Option<Spinner>,
    connected: Connected,
    available: Available,
    selected: Selected,
    connected_scroll_state: ScrollState,
    available_scroll_state: ScrollState,
    available_max_items: Cell<usize>,
    authenticate: Option<authenticate::Authenticate>,
    info: Option<Info>,
    profiles: Vec<ConnectionProfile>,
    active_connections: Vec<ActiveConnection>,
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
                        ("info", ("i", "info").into()),
                        ("tab", ("tab", "move to access points").into()),
                        ("scan", ("s", "force scan").into()),
                    ],
                    &["nav", "con", "info", "scan", "quit"],
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
                        ("info", ("i", "info").into()),
                        ("tab", ("tab", "move to connections").into()),
                        ("scan", ("s", "force scan").into()),
                    ],
                    &["nav", "con", "info", "scan", "quit"],
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
    pub fn new() -> Self {
        let connected = Connected { list: Vec::new() };
        let available = Available { list: Vec::new() };
        let selected = Selected::None;

        Self {
            loading: Some(Spinner::new(SpinnerType::dot(), 60)), // Default fps, will be updated by tick
            connected,
            available,
            selected,
            connected_scroll_state: ScrollState::new(),
            available_scroll_state: ScrollState::new(),
            available_max_items: Cell::new(5), // Default, will be updated by render
            authenticate: None,
            info: None,
            profiles: Vec::new(),
            active_connections: Vec::new(),
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

    fn rebuild_connected_list(&mut self) {
        let mut connected_list = Vec::new();
        
        // Add active connections first
        for active_conn in &self.active_connections {
            connected_list.push(Arc::new(map_active_to_connection_data(active_conn.clone())));
        }
        
        // Add inactive profiles (ones not currently active)
        for profile in &self.profiles {
            let is_active = self.active_connections.iter()
                .any(|ac| ac.profile_uuid == profile.uuid);
            if !is_active {
                connected_list.push(Arc::new(map_profile_to_inactive(profile.clone())));
            }
        }
        
        self.connected = Connected { list: connected_list };
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
        if let Some(ref mut info) = self.info {
            let bubble = info.update(ctx, ev);
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
                        if key_event.code.is_char('i') =>
                    {
                        let target = match self.selected {
                            Selected::Connected(idx) => {
                                Some(InfoTarget::Connection(self.connected.list[idx].clone()))
                            }
                            Selected::Available(idx) => {
                                Some(InfoTarget::Available(self.available.list[idx].clone()))
                            }
                            Selected::None => None,
                        };
                        if let Some(target) = target {
                            ctx.message.send(Message::OpenInfo(target)).unwrap();
                        }
                    }
                    ratatui::crossterm::event::Event::Key(key_event)
                        if key_event.code.is_char('s') =>
                    {
                        // Trigger scan on first WiFi device
                        // TODO: In a real implementation, we should get this from devices or context
                        ctx.message.send(Message::RequestScan {
                            interface: "wlan0".to_string(), // Default interface for now
                        }).unwrap();
                    }
                    ratatui::crossterm::event::Event::Key(key_event)
                        if key_event.code.is_enter() =>
                    {
                        if let Some(idx) = self.selected.connected_selected() {
                            // Handle connected list - disconnect active connections
                            if idx < self.active_connections.len() {
                                let active = &self.active_connections[idx];
                                ctx.message.send(Message::RequestDisconnect {
                                    active_id: active.id.clone(),
                                }).unwrap();
                            }
                        } else if let Some(s) = self.selected.available_selected()
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
                self.authenticate = Some(authenticate::Authenticate::new(
                    authentication_data.access_point,
                ));
            }
            Message::OpenInfo(target) => {
                self.info = Some(Info::new(target));
            }
            Message::FinishInfo => {
                self.info = None;
            }
            Message::BridgeResult(result) => {
                match result {
                    BridgeResult::ActiveConnections(Ok(active_conns)) => {
                        self.active_connections = active_conns;
                        self.rebuild_connected_list();
                    }
                    BridgeResult::Connections(Ok(profiles)) => {
                        self.profiles = profiles;
                        self.rebuild_connected_list();
                    }
                    BridgeResult::AccessPoints(Ok(aps)) => {
                        let available_list: Vec<_> = aps
                            .into_iter()
                            .map(|ap| Arc::new(map_ap_to_available_details(ap)))
                            .collect();
                        self.available = Available { list: available_list };
                    }
                    BridgeResult::AllDevices(Ok(_devices)) => {
                        // Devices loaded, handled by Application startup logic
                    }
                    BridgeResult::ConnectionAddedAndActivated(Ok(active)) => {
                        self.active_connections.push(active);
                        self.rebuild_connected_list();
                        self.authenticate = None;
                        self.loading = None;
                    }
                    BridgeResult::ConnectionActivated(Ok(active)) => {
                        self.active_connections.push(active);
                        self.rebuild_connected_list();
                        self.authenticate = None;
                        self.loading = None;
                    }
                    BridgeResult::ConnectionDeactivated { active_id, result: Ok(()) } => {
                        self.active_connections.retain(|ac| ac.id != *active_id);
                        self.rebuild_connected_list();
                    }
                    _ => {
                        // Handle other results or errors if needed
                        log::debug!("Unhandled bridge result: {:?}", result);
                    }
                }
            }
            Message::AuthSubmitted { .. } => {
                // Bubble up to Application so it can send the bridge command
                return Bubble::Yes(ev);
            }
            _ => {
                // User intent messages (RequestConnect, RequestScan, etc.) are handled
                // by Application before reaching Home. If they arrive here, ignore.
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
        if let Some(ref mut info) = self.info {
            info.draw(ctx, frame);
        }
    }
}
