use std::sync::Arc;
use std::time::Duration;

use circular_buffer::CircularBuffer;
use poppingboba::help::{HelpTable, HelpWidget};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    macros::{constraint, constraints, line, span, vertical},
    style::{Color, Style},
    text::Line,
    widgets::Widget,
};
use tui_overlay::{Backdrop, Overlay, OverlayState};

use crate::application::{
    component::{Bubble, Component, Message, RichContext},
    home::{available::AvailableAPDetails, connected::ConnectionData},
    theme::PATINA,
    utils::{
        BrailleSparkline, CowStr, Separator, humanize_duration, strength_bars, strength_color,
    },
};

pub enum InfoTarget {
    Connection(Arc<ConnectionData>),
    Available(Arc<AvailableAPDetails>),
}

pub struct Info {
    target: InfoTarget,
}

impl Info {
    pub fn new(target: InfoTarget) -> Self {
        Self { target }
    }
}

enum Section<'a> {
    Lines(Vec<Line<'static>>),
    Signal(&'a CircularBuffer<48, u8>),
    Throughput(&'a CircularBuffer<48, usize>, &'a CircularBuffer<48, usize>),
}

impl Section<'_> {
    fn height(&self) -> u16 {
        match self {
            Section::Lines(lines) => lines.len() as u16,
            Section::Signal(_) => 4,
            Section::Throughput(_, _) => 6,
        }
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        match self {
            Section::Lines(lines) => render_lines(frame, area, lines.clone()),
            Section::Signal(history) => render_signal_block(frame, area, history),
            Section::Throughput(rx, tx) => render_throughput_sparklines(frame, area, rx, tx),
        }
    }
}

impl Component for Info {
    fn update(&mut self, _ctx: &RichContext, ev: Message) -> Bubble {
        if let Message::Crossterm(ratatui::crossterm::event::Event::Key(event)) = &ev
            && event.kind == ratatui::crossterm::event::KeyEventKind::Press
            && (event.code.is_esc() || event.code.is_char('q') || event.code.is_char('i'))
        {
            return Bubble::Yes(Message::FinishInfo);
        }
        Bubble::No
    }

    fn draw(&mut self, _ctx: &RichContext, frame: &mut Frame<'_>) {
        let overlay = Overlay::new()
            .backdrop(Backdrop::new(PATINA.bg_alt))
            .width(constraint!(== 70 %))
            .height(constraint!(== 22))
            .anchor(tui_overlay::Anchor::Center);

        let mut overlay_state = OverlayState::default();
        overlay_state.open();

        let [overlay_area, _] = vertical![*= 1, == 2].areas(frame.area());

        frame.render_stateful_widget(overlay, overlay_area, &mut overlay_state);

        let inner = overlay_state
            .inner_area()
            .expect("overlay to be open as it is opened explicitly");

        let inner = inner.inner(ratatui::layout::Margin {
            horizontal: 2,
            vertical: 1,
        });

        let [
            title_area,
            subtitle_area,
            _,
            body_area,
            _,
            sep_area,
            _,
            footer_area,
        ] = Layout::vertical(constraints![
            == 1, == 1, == 1, *= 1, == 1, == 1, == 1, == 1
        ])
        .areas(inner);

        self.render_title(frame, title_area);
        self.render_subtitle(frame, subtitle_area);
        self.render_body(frame, body_area);

        Separator::new('\x5F')
            .styled(Style::new().fg(PATINA.mute))
            .render(sep_area, frame.buffer_mut());

        self.render_footer(frame, footer_area);
    }
}

impl Info {
    fn render_title(&self, frame: &mut Frame<'_>, area: Rect) {
        let title = span!(PATINA.accent; "INFO");
        let dot = span!(PATINA.dim; "  ·  ");

        let (name, signal) = match &self.target {
            InfoTarget::Connection(c) => match c.as_ref() {
                ConnectionData::WifiActive { name, strength, .. } => {
                    (name.clone(), Some(*strength))
                }
                ConnectionData::WiredActive { name, .. } => (name.clone(), None),
                ConnectionData::WifiInactive { name, .. } => (name.clone(), None),
            },
            InfoTarget::Available(ap) => (ap.ssid.clone(), Some(ap.strength)),
        };
        let name_span = span!(PATINA.fg; name);

        let left = if let Some(s) = signal {
            let bars = span!(strength_color(s); "{}", strength_bars(s));
            let pct = span!(PATINA.dim; " {}%", s.round() as u32);
            line![title, dot.clone(), name_span, dot, bars, pct]
        } else {
            line![title, dot, name_span]
        }
        .left_aligned();

        let right = self.status_line().right_aligned();

        right.render(area, frame.buffer_mut());
        left.render(area, frame.buffer_mut());
    }

    fn status_line(&self) -> Line<'static> {
        match &self.target {
            InfoTarget::Connection(c) => match c.as_ref() {
                ConnectionData::WifiActive { .. } | ConnectionData::WiredActive { .. } => {
                    line![span!(PATINA.live; "● "), span!(PATINA.live; "connected"),]
                }
                ConnectionData::WifiInactive { .. } => {
                    line![span!(PATINA.fg_alt; "○ "), span!(PATINA.fg_alt; "saved"),]
                }
            },
            InfoTarget::Available(_) => {
                line![span!(PATINA.fg_alt; "○ "), span!(PATINA.fg_alt; "visible"),]
            }
        }
    }

    fn render_subtitle(&self, frame: &mut Frame<'_>, area: Rect) {
        let sep = || span!(PATINA.dim; "  ·  ");
        let l = match &self.target {
            InfoTarget::Connection(c) => match c.as_ref() {
                ConnectionData::WifiActive {
                    uptime,
                    autoconnect,
                    metered,
                    ..
                } => active_subtitle(*uptime, *autoconnect, *metered),
                ConnectionData::WiredActive {
                    uptime,
                    autoconnect,
                    ..
                } => active_subtitle(*uptime, *autoconnect, false),
                ConnectionData::WifiInactive {
                    last_used,
                    autoconnect,
                    metered,
                    ..
                } => {
                    let mut spans = vec![
                        span!(PATINA.dim; "used "),
                        span!(PATINA.dim; humanize_duration(*last_used)),
                        sep(),
                        span!(PATINA.dim; "autoconnect "),
                        span!(PATINA.dim; if *autoconnect { "on" } else { "off" }),
                    ];
                    if *metered {
                        spans.push(sep());
                        spans.push(span!(PATINA.warn; "metered"));
                    }
                    Line::from(spans)
                }
            },
            InfoTarget::Available(ap) => {
                let mut spans = vec![
                    span!(PATINA.dim; ap.frequency.clone()),
                    sep(),
                    span!(PATINA.dim; "ch {}", ap.channel),
                    sep(),
                    span!(PATINA.dim; ap.security.clone()),
                ];
                if ap.saved {
                    spans.push(sep());
                    spans.push(span!(PATINA.accent; "saved"));
                }
                Line::from(spans)
            }
        };
        l.render(area, frame.buffer_mut());
    }

    fn render_body(&self, frame: &mut Frame<'_>, area: Rect) {
        let sections = self.build_sections();
        if sections.is_empty() || area.height == 0 {
            return;
        }
        let required = total_height(&sections);
        if required <= area.height {
            render_stack(frame, area, &sections);
        } else {
            render_two_column(frame, area, sections);
        }
    }

    fn build_sections(&self) -> Vec<Section<'_>> {
        match &self.target {
            InfoTarget::Connection(c) => match c.as_ref() {
                ConnectionData::WifiActive {
                    ip,
                    gateway,
                    dns,
                    mtu,
                    mac,
                    security,
                    bssid,
                    frequency,
                    channel,
                    link_speed,
                    signal_history,
                    rx_throughput,
                    tx_throughput,
                    ..
                } => vec![
                    Section::Lines(vec![
                        section_header("addressing"),
                        kv("ipv4", ip.clone()),
                        kv("gateway", gateway.clone()),
                        kv("dns", join_dns(dns)),
                        kv("mtu", mtu.to_string().into()),
                        kv("mac", mac.clone()),
                    ]),
                    Section::Lines(vec![
                        section_header("radio"),
                        kv("security", security.clone()),
                        kv("bssid", bssid.clone()),
                        kv("band", format!("{}  ·  ch {}", frequency, channel).into()),
                        kv("link speed", link_speed.clone()),
                    ]),
                    Section::Signal(signal_history),
                    Section::Throughput(rx_throughput, tx_throughput),
                ],
                ConnectionData::WiredActive {
                    ip,
                    gateway,
                    dns,
                    mtu,
                    mac,
                    link_speed,
                    interface,
                    rx_throughput,
                    tx_throughput,
                    ..
                } => vec![
                    Section::Lines(vec![
                        section_header("addressing"),
                        kv("ipv4", ip.clone()),
                        kv("gateway", gateway.clone()),
                        kv("dns", join_dns(dns)),
                        kv("mtu", mtu.to_string().into()),
                        kv("mac", mac.clone()),
                        kv("iface", format!("{}  ·  {}", interface, link_speed).into()),
                    ]),
                    Section::Throughput(rx_throughput, tx_throughput),
                ],
                ConnectionData::WifiInactive {
                    security,
                    bssid,
                    saved_ip_method,
                    saved_dns,
                    autoconnect,
                    metered,
                    ..
                } => vec![
                    Section::Lines(vec![
                        section_header("security"),
                        kv("method", security.clone()),
                        kv("bssid", bssid.clone().unwrap_or_else(|| "—".into())),
                    ]),
                    Section::Lines(vec![
                        section_header("saved settings"),
                        kv("ipv4", saved_ip_method.clone()),
                        kv("dns", join_dns(saved_dns)),
                        kv(
                            "autoconnect",
                            if *autoconnect {
                                "on".into()
                            } else {
                                "off".into()
                            },
                        ),
                        kv("metered", if *metered { "yes".into() } else { "no".into() }),
                    ]),
                ],
            },
            InfoTarget::Available(ap) => vec![
                Section::Lines(vec![
                    section_header("access point"),
                    kv("ssid", ap.ssid.clone()),
                    kv("security", ap.security.clone()),
                    kv("bssid", ap.bssid.clone()),
                    kv(
                        "band",
                        format!("{}  ·  ch {}", ap.frequency, ap.channel).into(),
                    ),
                    kv("link speed", ap.link_speed.clone()),
                    kv(
                        "profile",
                        if ap.saved {
                            "saved".into()
                        } else {
                            "new".into()
                        },
                    ),
                ]),
                Section::Signal(&ap.signal_history),
            ],
        }
    }

    fn render_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        let help = match &self.target {
            InfoTarget::Connection(c) => match c.as_ref() {
                ConnectionData::WifiActive { .. } | ConnectionData::WiredActive { .. } => {
                    HelpWidget::new(HelpTable::new(
                        [
                            ("disc", ("enter", "disconnect").into()),
                            ("edit", ("e", "edit").into()),
                            ("forget", ("d", "forget").into()),
                            ("close", ("esc", "close").into()),
                        ],
                        &["disc", "edit", "forget", "close"],
                        (&[], 0),
                    ))
                }
                ConnectionData::WifiInactive { .. } => HelpWidget::new(HelpTable::new(
                    [
                        ("conn", ("enter", "connect").into()),
                        ("edit", ("e", "edit").into()),
                        ("forget", ("d", "forget").into()),
                        ("close", ("esc", "close").into()),
                    ],
                    &["conn", "edit", "forget", "close"],
                    (&[], 0),
                )),
            },
            InfoTarget::Available(_) => HelpWidget::new(HelpTable::new(
                [
                    ("conn", ("enter", "connect").into()),
                    ("close", ("esc", "close").into()),
                ],
                &["conn", "close"],
                (&[], 0),
            )),
        };
        help.render(area, frame.buffer_mut());
    }
}

fn total_height(sections: &[Section<'_>]) -> u16 {
    let body: u16 = sections.iter().map(|s| s.height()).sum();
    let gutters = sections.len().saturating_sub(1) as u16;
    body + gutters
}

fn render_stack(frame: &mut Frame<'_>, area: Rect, sections: &[Section<'_>]) {
    if sections.is_empty() {
        return;
    }
    let mut constraints: Vec<Constraint> = Vec::with_capacity(sections.len() * 2);
    for (i, s) in sections.iter().enumerate() {
        if i > 0 {
            constraints.push(constraint!(== 1));
        }
        constraints.push(constraint!(== s.height()));
    }
    constraints.push(constraint!(*= 1));
    let rects = Layout::vertical(constraints).split(area);
    let mut idx = 0;
    for (i, s) in sections.iter().enumerate() {
        if i > 0 {
            idx += 1;
        }
        s.render(frame, rects[idx]);
        idx += 1;
    }
}

fn render_two_column(frame: &mut Frame<'_>, area: Rect, sections: Vec<Section<'_>>) {
    let [left, _gap, right] = Layout::horizontal(constraints![*= 1, == 2, *= 1]).areas(area);

    let max_h = area.height;
    let mut left_sections: Vec<Section<'_>> = Vec::new();
    let mut right_sections: Vec<Section<'_>> = Vec::new();
    let mut left_filled: u16 = 0;
    let mut packing_left = true;

    for s in sections {
        if packing_left {
            let added = s.height() + if left_sections.is_empty() { 0 } else { 1 };
            if left_filled + added <= max_h {
                left_filled += added;
                left_sections.push(s);
                continue;
            }
            packing_left = false;
        }
        right_sections.push(s);
    }

    render_stack(frame, left, &left_sections);
    render_stack(frame, right, &right_sections);
}

fn active_subtitle(uptime: Duration, autoconnect: bool, metered: bool) -> Line<'static> {
    let sep = || span!(PATINA.dim; "  ·  ");
    let mut spans = vec![
        span!(PATINA.dim; "uptime "),
        span!(PATINA.dim; humanize_duration_short(uptime)),
        sep(),
        span!(PATINA.dim; "autoconnect "),
        span!(PATINA.dim; if autoconnect { "on" } else { "off" }),
    ];
    if metered {
        spans.push(sep());
        spans.push(span!(PATINA.warn; "metered"));
    }
    Line::from(spans)
}

fn section_header(label: &'static str) -> Line<'static> {
    Line::from(span!(PATINA.accent; label))
}

fn kv(label: &'static str, value: CowStr) -> Line<'static> {
    let label_s = span!(PATINA.dim; "{:<12}", label);
    let val_s = span!(PATINA.fg; value);
    Line::from(vec![label_s, val_s])
}

fn join_dns(dns: &[CowStr]) -> CowStr {
    if dns.is_empty() {
        "—".into()
    } else {
        dns.iter()
            .map(|s| s.as_ref())
            .collect::<Vec<_>>()
            .join(", ")
            .into()
    }
}

fn render_lines(frame: &mut Frame<'_>, area: Rect, lines: Vec<Line<'_>>) {
    let n = lines.len() as u16;
    if n == 0 || area.height == 0 {
        return;
    }
    let count = n.min(area.height);
    let constraints: Vec<_> = std::iter::repeat_n(constraint!(== 1), count as usize).collect();
    let rects = Layout::vertical(constraints).split(Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: count,
    });
    for (line, rect) in lines
        .into_iter()
        .take(count as usize)
        .zip(rects.iter().cloned())
    {
        line.render(rect, frame.buffer_mut());
    }
}

fn render_signal_block(frame: &mut Frame<'_>, area: Rect, history: &CircularBuffer<48, u8>) {
    if area.height < 4 {
        return;
    }
    let [header_area, spark_area, stats_area, _] =
        Layout::vertical(constraints![== 1, == 2, == 1, *= 1]).areas(area);

    section_header("live signal").render(header_area, frame.buffer_mut());

    let data: Vec<usize> = history.iter().map(|&v| v as usize).collect();
    BrailleSparkline::new(&data)
        .max(100)
        .style(Style::new().fg(PATINA.live))
        .render(spark_area, frame.buffer_mut());

    if data.is_empty() {
        return;
    }
    let min = *data.iter().min().unwrap();
    let max = *data.iter().max().unwrap();
    let avg = data.iter().sum::<usize>() / data.len();
    let stats = line![
        span!(PATINA.dim; "min "),
        span!(PATINA.fg; "{:>3}%", min),
        span!(PATINA.dim; "  ·  avg "),
        span!(PATINA.fg; "{:>3}%", avg),
        span!(PATINA.dim; "  ·  max "),
        span!(PATINA.fg; "{:>3}%", max),
    ];
    stats.render(stats_area, frame.buffer_mut());
}

fn render_throughput_sparklines(
    frame: &mut Frame<'_>,
    area: Rect,
    rx: &CircularBuffer<48, usize>,
    tx: &CircularBuffer<48, usize>,
) {
    if area.height < 5 {
        return;
    }
    let [header_area, body_area] = Layout::vertical(constraints![== 1, *= 1]).areas(area);
    section_header("throughput").render(header_area, frame.buffer_mut());

    let [left, _gap, right] = Layout::horizontal(constraints![*= 1, == 2, *= 1]).areas(body_area);

    let [up_label, up_spark, _, up_stats] =
        Layout::vertical(constraints![== 1, == 2, == 1, == 1]).areas(left);
    let [dn_label, dn_spark, _, dn_stats] =
        Layout::vertical(constraints![== 1, == 2, == 1, == 1]).areas(right);

    line![span!(PATINA.accent; "▲ "), span!(PATINA.dim; "upload"),]
        .render(up_label, frame.buffer_mut());
    line![span!(PATINA.live; "▼ "), span!(PATINA.dim; "download"),]
        .render(dn_label, frame.buffer_mut());

    let tx_data: Vec<usize> = tx.iter().copied().collect();
    let rx_data: Vec<usize> = rx.iter().copied().collect();

    BrailleSparkline::new(&tx_data)
        .style(Style::new().fg(PATINA.accent))
        .render(up_spark, frame.buffer_mut());
    BrailleSparkline::new(&rx_data)
        .style(Style::new().fg(PATINA.live))
        .render(dn_spark, frame.buffer_mut());

    let stat_line = |peak: usize, color: Color| -> Line<'static> {
        line![span!(PATINA.dim; "peak "), span!(color; "{} KiB/s", peak),]
    };
    let tx_peak = tx_data.iter().copied().max().unwrap_or(0);
    let rx_peak = rx_data.iter().copied().max().unwrap_or(0);
    stat_line(tx_peak, PATINA.accent).render(up_stats, frame.buffer_mut());
    stat_line(rx_peak, PATINA.live).render(dn_stats, frame.buffer_mut());
}

fn humanize_duration_short(d: Duration) -> String {
    let s = d.as_secs();
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h >= 24 {
        format!("{}d {}h", h / 24, h % 24)
    } else if h > 0 {
        format!("{}h {}m", h, m)
    } else if m > 0 {
        format!("{}m {}s", m, sec)
    } else {
        format!("{}s", sec)
    }
}
