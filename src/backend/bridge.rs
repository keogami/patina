use std::collections::HashMap;
use uuid::Uuid;

use crate::backend::types::*;

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