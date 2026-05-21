use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::application::component::Message;
use crate::backend::types::*;
use crate::backend::NetworkManagerBackend;

/// Cloneable error type for bridge results
#[derive(Debug, Clone)]
pub struct BridgeError {
    pub message: String,
}

impl From<anyhow::Error> for BridgeError {
    fn from(err: anyhow::Error) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

pub type BridgeResultType<T> = Result<T, BridgeError>;

/// Commands sent from the UI thread to the bridge task
#[derive(Debug, Clone)]
pub enum BridgeCommand {
    // Discovery
    GetAllDevices,
    GetDeviceByInterface { interface: String },
    GetAccessPoints { interface: String },
    RequestScan { interface: String },

    // Connection Control
    ListConnections,
    GetActiveConnections,
    GetConnectionByUuid { uuid: Uuid },
    ActivateConnection { uuid: Uuid, interface: Option<String> },
    DeactivateConnection { active_id: String },
    DisconnectDevice { interface: String },
    AddAndActivateConnection { settings: ConnectionSettings, interface: Option<String> },
    GetSecrets { uuid: Uuid, setting_name: String },

    // Configuration (CRUD)
    GetSettings { uuid: Uuid },
    UpdateConnection { uuid: Uuid, settings: ConnectionSettings },
    AddConnection { settings: ConnectionSettings },
    DeleteConnection { uuid: Uuid },

    // Global State
    GetGlobalState,
    SetNetworkingEnabled { enabled: bool },
    SetWirelessEnabled { enabled: bool },
    SetWwanEnabled { enabled: bool },
    SaveHostname { hostname: String },

    // Device Settings
    SetDeviceManaged { interface: String, managed: bool },
    SetDeviceAutoconnect { interface: String, autoconnect: bool },

    // Monitoring
    GetDeviceStats { interface: String },

    // Subscriptions
    SubscribeDeviceEvents,
    SubscribeApEvents { interface: String },
    SubscribeProfileEvents,
    SubscribeActiveEvents,
    Unsubscribe { handle: SubscriptionHandle },
}

/// Results sent from the bridge task back to the UI thread
#[derive(Debug, Clone)]
pub enum BridgeResult {
    // Discovery results
    AllDevices(BridgeResultType<Vec<Device>>),
    DeviceByInterface(BridgeResultType<Option<Device>>),
    AccessPoints(BridgeResultType<Vec<AccessPoint>>),
    ScanRequested { interface: String, result: BridgeResultType<()> },

    // Connection Control results
    Connections(BridgeResultType<Vec<ConnectionProfile>>),
    ActiveConnections(BridgeResultType<Vec<ActiveConnection>>),
    ConnectionByUuid(BridgeResultType<ConnectionProfile>),
    ConnectionActivated(BridgeResultType<ActiveConnection>),
    ConnectionDeactivated { active_id: String, result: BridgeResultType<()> },
    DeviceDisconnected(BridgeResultType<()>),
    ConnectionAddedAndActivated(BridgeResultType<ActiveConnection>),
    Secrets(BridgeResultType<HashMap<String, String>>),

    // Configuration (CRUD) results
    Settings(BridgeResultType<ConnectionSettings>),
    ConnectionUpdated(BridgeResultType<()>),
    ConnectionAdded(BridgeResultType<ConnectionProfile>),
    ConnectionDeleted(BridgeResultType<()>),

    // Global State results
    GlobalState(BridgeResultType<GlobalState>),
    NetworkingEnabledSet(BridgeResultType<()>),
    WirelessEnabledSet(BridgeResultType<()>),
    WwanEnabledSet(BridgeResultType<()>),
    HostnameSaved(BridgeResultType<()>),

    // Device Settings results
    DeviceManagedSet(BridgeResultType<()>),
    DeviceAutoconnectSet(BridgeResultType<()>),

    // Monitoring results
    DeviceStats(BridgeResultType<DeviceStats>),

    // Subscription results
    DeviceEventsSubscribed(BridgeResultType<SubscriptionHandle>),
    ApEventsSubscribed(BridgeResultType<SubscriptionHandle>),
    ProfileEventsSubscribed(BridgeResultType<SubscriptionHandle>),
    ActiveEventsSubscribed(BridgeResultType<SubscriptionHandle>),

    // Subscription push events (sent when events are received)
    DeviceEvent(DeviceEvent),
    ApEvent(ApEvent),
    ProfileEvent(ProfileEvent),
    ActiveEvent(ActiveEvent),
}

/// Opaque handle for managing subscriptions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionHandle(pub u64);

/// Bridge between UI thread and async backend.
///
/// Spawns a single tokio coordinator task that dispatches each command
/// as a detached child tokio task. Subscriptions are cancellable.
#[derive(Clone)]
pub struct BackendBridge {
    cmd_tx: mpsc::UnboundedSender<BridgeCommand>,
}

impl BackendBridge {
    pub fn new<B: NetworkManagerBackend>(
        backend: B,
        ui_tx: std::sync::mpsc::Sender<Message>,
    ) -> Self {
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut subscriptions: HashMap<SubscriptionHandle, CancellationToken> = HashMap::new();
            let mut next_handle: u64 = 0;
            let ui_tx = Arc::new(ui_tx);

            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    // ── One-shot discovery ──
                    BridgeCommand::GetAllDevices => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_all_devices().await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::AllDevices(res)));
                        });
                    }
                    BridgeCommand::GetDeviceByInterface { interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_device_by_interface(&interface)
                                .await
                                .map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::DeviceByInterface(res)));
                        });
                    }
                    BridgeCommand::GetAccessPoints { interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_access_points(&interface)
                                .await
                                .map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::AccessPoints(res)));
                        });
                    }
                    BridgeCommand::RequestScan { interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        let iface = interface.clone();
                        tokio::spawn(async move {
                            let res = b.request_scan(&interface).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ScanRequested {
                                interface: iface,
                                result: res,
                            }));
                        });
                    }

                    // ── One-shot connection control ──
                    BridgeCommand::ListConnections => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.list_connections().await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::Connections(res)));
                        });
                    }
                    BridgeCommand::GetActiveConnections => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_active_connections().await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ActiveConnections(res)));
                        });
                    }
                    BridgeCommand::GetConnectionByUuid { uuid } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_connection_by_uuid(&uuid).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionByUuid(res)));
                        });
                    }
                    BridgeCommand::ActivateConnection { uuid, interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.activate_connection(&uuid, interface.as_deref())
                                .await
                                .map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionActivated(res)));
                        });
                    }
                    BridgeCommand::DeactivateConnection { active_id } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        let id = active_id.clone();
                        tokio::spawn(async move {
                            let res = b.deactivate_connection(&active_id)
                                .await
                                .map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionDeactivated {
                                active_id: id,
                                result: res,
                            }));
                        });
                    }
                    BridgeCommand::DisconnectDevice { interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.disconnect_device(&interface).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::DeviceDisconnected(res)));
                        });
                    }
                    BridgeCommand::AddAndActivateConnection { settings, interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.add_and_activate_connection(&settings, interface.as_deref())
                                .await
                                .map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionAddedAndActivated(res)));
                        });
                    }
                    BridgeCommand::GetSecrets { uuid, setting_name } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_secrets(&uuid, &setting_name).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::Secrets(res)));
                        });
                    }

                    // ── Configuration ──
                    BridgeCommand::GetSettings { uuid } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_settings(&uuid).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::Settings(res)));
                        });
                    }
                    BridgeCommand::UpdateConnection { uuid, settings } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.update_connection(&uuid, &settings).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionUpdated(res)));
                        });
                    }
                    BridgeCommand::AddConnection { settings } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.add_connection(&settings).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionAdded(res)));
                        });
                    }
                    BridgeCommand::DeleteConnection { uuid } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.delete_connection(&uuid).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionDeleted(res)));
                        });
                    }

                    // ── Global state ──
                    BridgeCommand::GetGlobalState => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_global_state().await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::GlobalState(res)));
                        });
                    }
                    BridgeCommand::SetNetworkingEnabled { enabled } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.set_networking_enabled(enabled).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::NetworkingEnabledSet(res)));
                        });
                    }
                    BridgeCommand::SetWirelessEnabled { enabled } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.set_wireless_enabled(enabled).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::WirelessEnabledSet(res)));
                        });
                    }
                    BridgeCommand::SetWwanEnabled { enabled } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.set_wwan_enabled(enabled).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::WwanEnabledSet(res)));
                        });
                    }
                    BridgeCommand::SaveHostname { hostname } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.save_hostname(&hostname).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::HostnameSaved(res)));
                        });
                    }

                    // ── Device settings ──
                    BridgeCommand::SetDeviceManaged { interface, managed } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.set_device_managed(&interface, managed)
                                .await
                                .map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::DeviceManagedSet(res)));
                        });
                    }
                    BridgeCommand::SetDeviceAutoconnect { interface, autoconnect } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.set_device_autoconnect(&interface, autoconnect)
                                .await
                                .map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::DeviceAutoconnectSet(res)));
                        });
                    }

                    // ── Stats ──
                    BridgeCommand::GetDeviceStats { interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_device_stats(&interface).await.map_err(BridgeError::from);
                            let _ = tx.send(Message::BridgeResult(BridgeResult::DeviceStats(res)));
                        });
                    }

                    // ── Subscriptions ──
                    BridgeCommand::SubscribeDeviceEvents => {
                        let handle = SubscriptionHandle(next_handle);
                        next_handle += 1;
                        let token = CancellationToken::new();
                        let child_token = token.clone();
                        subscriptions.insert(handle, token);

                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            match b.subscribe_device_events().await {
                                Ok(mut rx) => {
                                    let _ = tx.send(Message::BridgeResult(
                                        BridgeResult::DeviceEventsSubscribed(Ok(handle)),
                                    ));
                                    loop {
                                        tokio::select! {
                                            _ = child_token.cancelled() => break,
                                            Some(ev) = rx.recv() => {
                                                let _ = tx.send(Message::BridgeResult(
                                                    BridgeResult::DeviceEvent(ev),
                                                ));
                                            }
                                            else => break,
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("subscribe_device_events failed: {}", e);
                                    let _ = tx.send(Message::BridgeResult(
                                        BridgeResult::DeviceEventsSubscribed(Err(BridgeError::from(e))),
                                    ));
                                }
                            }
                        });
                    }
                    BridgeCommand::SubscribeApEvents { interface } => {
                        let handle = SubscriptionHandle(next_handle);
                        next_handle += 1;
                        let token = CancellationToken::new();
                        let child_token = token.clone();
                        subscriptions.insert(handle, token);

                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            match b.subscribe_ap_events(&interface).await {
                                Ok(mut rx) => {
                                    let _ = tx.send(Message::BridgeResult(
                                        BridgeResult::ApEventsSubscribed(Ok(handle)),
                                    ));
                                    loop {
                                        tokio::select! {
                                            _ = child_token.cancelled() => break,
                                            Some(ev) = rx.recv() => {
                                                let _ = tx.send(Message::BridgeResult(
                                                    BridgeResult::ApEvent(ev),
                                                ));
                                            }
                                            else => break,
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("subscribe_ap_events failed: {}", e);
                                    let _ = tx.send(Message::BridgeResult(
                                        BridgeResult::ApEventsSubscribed(Err(BridgeError::from(e))),
                                    ));
                                }
                            }
                        });
                    }
                    BridgeCommand::SubscribeProfileEvents => {
                        let handle = SubscriptionHandle(next_handle);
                        next_handle += 1;
                        let token = CancellationToken::new();
                        let child_token = token.clone();
                        subscriptions.insert(handle, token);

                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            match b.subscribe_profile_events().await {
                                Ok(mut rx) => {
                                    let _ = tx.send(Message::BridgeResult(
                                        BridgeResult::ProfileEventsSubscribed(Ok(handle)),
                                    ));
                                    loop {
                                        tokio::select! {
                                            _ = child_token.cancelled() => break,
                                            Some(ev) = rx.recv() => {
                                                let _ = tx.send(Message::BridgeResult(
                                                    BridgeResult::ProfileEvent(ev),
                                                ));
                                            }
                                            else => break,
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("subscribe_profile_events failed: {}", e);
                                    let _ = tx.send(Message::BridgeResult(
                                        BridgeResult::ProfileEventsSubscribed(Err(BridgeError::from(e))),
                                    ));
                                }
                            }
                        });
                    }
                    BridgeCommand::SubscribeActiveEvents => {
                        let handle = SubscriptionHandle(next_handle);
                        next_handle += 1;
                        let token = CancellationToken::new();
                        let child_token = token.clone();
                        subscriptions.insert(handle, token);

                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            match b.subscribe_active_events().await {
                                Ok(mut rx) => {
                                    let _ = tx.send(Message::BridgeResult(
                                        BridgeResult::ActiveEventsSubscribed(Ok(handle)),
                                    ));
                                    loop {
                                        tokio::select! {
                                            _ = child_token.cancelled() => break,
                                            Some(ev) = rx.recv() => {
                                                let _ = tx.send(Message::BridgeResult(
                                                    BridgeResult::ActiveEvent(ev),
                                                ));
                                            }
                                            else => break,
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("subscribe_active_events failed: {}", e);
                                    let _ = tx.send(Message::BridgeResult(
                                        BridgeResult::ActiveEventsSubscribed(Err(BridgeError::from(e))),
                                    ));
                                }
                            }
                        });
                    }
                    BridgeCommand::Unsubscribe { handle } => {
                        if let Some(token) = subscriptions.remove(&handle) {
                            token.cancel();
                        }
                    }
                }
            }
        });

        Self { cmd_tx }
    }

    pub fn send(&self, cmd: BridgeCommand) {
        let _ = self.cmd_tx.send(cmd);
    }
}
