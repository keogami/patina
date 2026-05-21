use std::{io::stdout, sync::Arc, time::Duration};

use anyhow::Context as _;
use enum_dispatch::enum_dispatch;
use ratatui::{
    Frame,
    crossterm::{self},
    style::Color,
};

use crate::application::{
    component::{Bubble, Component, Message, RichContext},
    home::HomeData,
    theme::PATINA,
};
use crate::backend::bridge::{BackendBridge, BridgeCommand};

pub mod component;
mod home;
mod theme;
pub mod utils;

pub use component::Context;

pub struct Application {
    context: Arc<RichContext>,
    pane: Pane,
    message_tx: Arc<std::sync::mpsc::Sender<Message>>,
    message_rx: Option<std::sync::mpsc::Receiver<Message>>,
    bridge: Arc<BackendBridge>,
    startup_pending: u32,
}

impl Application {
    pub fn new(
        context: Arc<RichContext>,
        bridge: Arc<BackendBridge>,
        message_tx: Arc<std::sync::mpsc::Sender<Message>>,
        message_rx: std::sync::mpsc::Receiver<Message>,
    ) -> Self {
        Self {
            context,
            pane: Pane::Home(HomeData::new()),
            message_tx,
            message_rx: Some(message_rx),
            bridge,
            startup_pending: 0,
        }
    }

    pub fn setup_bg(&self) -> anyhow::Result<()> {
        // TODO: Look for better conversion from ratatui::Color to crossterm::Color
        let (r, g, b) = match PATINA.bg {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => return Ok(()),
        };

        // TODO: make this robust
        // - not all terms allow this OSC 11 calls
        // - terms dont reset on exit
        let command = crossterm::style::Print(format!("\x1b]11;#{r:02x}{g:02x}{b:02x}\x07"));

        crossterm::execute!(stdout(), command).context("Couldn't set bg color")?;

        let clear = crossterm::terminal::Clear(crossterm::terminal::ClearType::All);
        crossterm::execute!(stdout(), clear).context("Couldn't clear bg")?;

        Ok(())
    }

    pub fn setup_event_polling(&self) -> anyhow::Result<()> {
        let crossterm_tx = self.message_tx.clone();
        std::thread::spawn(move || {
            loop {
                // TODO: catch this error and only panic if N consecutive calls return an error
                // also write an error! log on error
                let ev = crossterm::event::read().unwrap();

                crossterm_tx.send(Message::Crossterm(ev)).unwrap();
            }
        });

        let timer_tx = self.message_tx.clone();
        let interval = Duration::from_secs(1).div_f64(self.context.fps as _);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let _ = timer_tx.send(Message::GlobalTick);
            }
        });

        Ok(())
    }

    /// runs the main loop for the application
    pub fn run(mut self) -> anyhow::Result<()> {
        self.setup_event_polling()?;
        
        // Send initial startup commands
        self.bridge.send(BridgeCommand::GetAllDevices);
        self.bridge.send(BridgeCommand::ListConnections);
        self.bridge.send(BridgeCommand::GetActiveConnections);
        self.startup_pending = 3;
        
        let rx = self.message_rx.take().unwrap();

        ratatui::run(|term| {
            self.setup_bg()?;
            for msg in rx {
                term.draw(|frame| {
                    self.draw(frame);
                })?;

                if let Message::Crossterm(crossterm::event::Event::Key(key)) = msg
                    && key.code.is_char('q')
                {
                    break;
                }

                self.update(msg);
            }

            anyhow::Ok(())
        })?;

        Ok(())
    }

    pub fn update(&mut self, ev: Message) {
        // Intercept user intent messages and convert to bridge commands
        match &ev {
            Message::RequestConnect { uuid, interface } => {
                self.bridge.send(BridgeCommand::ActivateConnection {
                    uuid: *uuid,
                    interface: interface.clone(),
                });
                return;
            }
            Message::RequestDisconnect { active_id } => {
                self.bridge.send(BridgeCommand::DeactivateConnection {
                    active_id: active_id.clone(),
                });
                return;
            }
            Message::RequestScan { interface } => {
                self.bridge.send(BridgeCommand::RequestScan {
                    interface: interface.clone(),
                });
                return;
            }
            Message::RequestDeviceList => {
                self.bridge.send(BridgeCommand::GetAllDevices);
                return;
            }
            Message::RequestConnectionList => {
                self.bridge.send(BridgeCommand::ListConnections);
                return;
            }
            Message::AuthSubmitted { ssid, password, autoconnect: _ } => {
                self.bridge.send(BridgeCommand::AddAndActivateConnection {
                    settings: crate::backend::types::ConnectionSettings {
                        common: crate::backend::types::CommonSettings {
                            name: ssid.clone(),
                            uuid: uuid::Uuid::new_v4(),
                            autoconnect: true,
                            interface_name: None,
                            cloned_mac: None,
                            mtu: None,
                            metered: None,
                            zone: None,
                        },
                        connection_type: crate::backend::types::ConnectionTypeSettings::Wifi {
                            ssid: ssid.as_bytes().to_vec(),
                            mode: crate::backend::types::WifiMode::Infra,
                            bssid: None,
                            band: None,
                            channel: None,
                            security: crate::backend::types::WifiSecuritySettings::WpaPsk {
                                version: crate::backend::types::WpaVersion::Wpa2,
                                password: password.clone(),
                            },
                            hidden: false,
                            powersave: None,
                        },
                        ipv4: crate::backend::types::IpSettings {
                            method: crate::backend::types::IpMethod::Auto,
                            addresses: vec![],
                            dns: vec![],
                            gateway: None,
                            search_domains: vec![],
                            routes: vec![],
                            never_default: false,
                            ignore_auto_dns: false,
                        },
                        ipv6: crate::backend::types::IpSettings {
                            method: crate::backend::types::IpMethod::Auto,
                            addresses: vec![],
                            dns: vec![],
                            gateway: None,
                            search_domains: vec![],
                            routes: vec![],
                            never_default: false,
                            ignore_auto_dns: false,
                        },
                    },
                    interface: None, // TODO: get from context
                });
                self.pane.update(&self.context, Message::FinishAuthentication);
                return;
            }
            Message::RequestAddAndActivate { settings, interface } => {
                self.bridge.send(BridgeCommand::AddAndActivateConnection {
                    settings: settings.clone(),
                    interface: interface.clone(),
                });
                return;
            }
            Message::BridgeResult(result) => {
                // Track startup completion
                match result {
                    crate::backend::bridge::BridgeResult::AllDevices(Ok(devices)) => {
                        self.startup_pending = self.startup_pending.saturating_sub(1);
                        
                        // Request access points from WiFi devices
                        for device in devices {
                            if matches!(device.device_type, crate::backend::DeviceType::Wifi) {
                                self.bridge.send(BridgeCommand::GetAccessPoints {
                                    interface: device.interface.clone(),
                                });
                                self.startup_pending += 1; // Increment for each AP request
                            }
                        }
                    }
                    crate::backend::bridge::BridgeResult::ActiveConnections(_) => {
                        self.startup_pending = self.startup_pending.saturating_sub(1);
                    }
                    crate::backend::bridge::BridgeResult::Connections(_) => {
                        self.startup_pending = self.startup_pending.saturating_sub(1);
                    }
                    crate::backend::bridge::BridgeResult::AccessPoints(_) => {
                        self.startup_pending = self.startup_pending.saturating_sub(1);
                    }
                    crate::backend::bridge::BridgeResult::ScanRequested { interface, result: Ok(()) } => {
                        // After scan completes, refresh AP list
                        self.bridge.send(BridgeCommand::GetAccessPoints {
                            interface: interface.clone(),
                        });
                    }
                    _ => {}
                }
                
                // Check if startup is complete
                if self.startup_pending == 0 {
                    let _ = self.context.message.send(Message::FinishLoading);
                }
                
                // Forward to pane for UI updates
                self.pane.update(&self.context, ev);
                return;
            }
            _ => {}
        }
        
        // Forward all other messages to the pane
        self.pane.update(&self.context, ev);
    }

    pub fn draw(&mut self, frame: &mut Frame<'_>) {
        self.pane.draw(&self.context, frame);
    }
}

#[enum_dispatch(Component)]
pub enum Pane {
    Home(HomeData),
}
