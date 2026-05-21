use std::collections::HashMap;
use std::sync::{Arc, mpsc};
use uuid::Uuid;

use crate::backend::{types::*, NetworkManagerBackend};
use crate::application::component::Message;

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
    ScanRequested(BridgeResultType<()>),

    // Connection Control results
    Connections(BridgeResultType<Vec<ConnectionProfile>>),
    ActiveConnections(BridgeResultType<Vec<ActiveConnection>>),
    ConnectionByUuid(BridgeResultType<ConnectionProfile>),
    ConnectionActivated(BridgeResultType<ActiveConnection>),
    ConnectionDeactivated(BridgeResultType<()>),
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

/// Bridge between UI thread and backend thread
pub struct BackendBridge {
    command_tx: mpsc::Sender<BridgeCommand>,
}

impl BackendBridge {
    pub fn new<B: NetworkManagerBackend + Send + 'static>(
        backend: B,
        ui_tx: Arc<mpsc::Sender<Message>>,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<BridgeCommand>();

        // Spawn backend thread
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime in bridge thread");

            rt.block_on(async {
                for command in command_rx {
                    let result = Self::handle_command(&backend, command).await;
                    if let Err(e) = ui_tx.send(Message::BridgeResult(result)) {
                        log::error!("Failed to send bridge result to UI: {}", e);
                        break;
                    }
                }
            });
        });

        Self { command_tx }
    }

    pub fn send(&self, command: BridgeCommand) -> Result<(), BridgeError> {
        self.command_tx
            .send(command)
            .map_err(|_| BridgeError {
                message: "Bridge channel closed".to_string(),
            })
    }

    async fn handle_command<B: NetworkManagerBackend>(
        backend: &B,
        command: BridgeCommand,
    ) -> BridgeResult {
        match command {
            BridgeCommand::GetAllDevices => {
                let result = backend.get_all_devices().await.map_err(BridgeError::from);
                BridgeResult::AllDevices(result)
            }
            BridgeCommand::GetActiveConnections => {
                let result = backend.get_active_connections().await.map_err(BridgeError::from);
                BridgeResult::ActiveConnections(result)
            }
            BridgeCommand::GetAccessPoints { interface } => {
                let result = backend
                    .get_access_points(&interface)
                    .await
                    .map_err(BridgeError::from);
                BridgeResult::AccessPoints(result)
            }
            BridgeCommand::RequestScan { interface } => {
                let result = backend
                    .request_scan(&interface)
                    .await
                    .map_err(BridgeError::from);
                BridgeResult::ScanRequested(result)
            }
            BridgeCommand::ListConnections => {
                let result = backend.list_connections().await.map_err(BridgeError::from);
                BridgeResult::Connections(result)
            }
            BridgeCommand::ActivateConnection { uuid, interface } => {
                let result = backend
                    .activate_connection(&uuid, interface.as_deref())
                    .await
                    .map_err(BridgeError::from);
                BridgeResult::ConnectionActivated(result)
            }
            BridgeCommand::DeactivateConnection { active_id } => {
                let result = backend
                    .deactivate_connection(&active_id)
                    .await
                    .map_err(BridgeError::from);
                BridgeResult::ConnectionDeactivated(result)
            }
            BridgeCommand::AddAndActivateConnection { settings, interface } => {
                let result = backend
                    .add_and_activate_connection(&settings, interface.as_deref())
                    .await
                    .map_err(BridgeError::from);
                BridgeResult::ConnectionAddedAndActivated(result)
            }
            // Add other command handlers as needed
            _ => {
                // For unimplemented commands, return a placeholder error
                let error = BridgeError {
                    message: "Command not yet implemented".to_string(),
                };
                // Return appropriate result variant based on command type
                BridgeResult::AllDevices(Err(error))
            }
        }
    }
}