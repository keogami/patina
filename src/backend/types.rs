use std::collections::HashMap;
use std::net::IpAddr;
use std::time::SystemTime;
use uuid::Uuid;

// =================== Enums ===================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Unknown,
    Ethernet,
    Wifi,
    Bond,
    Bridge,
    Vlan,
    Team,
    Infiniband,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Unknown,
    Unmanaged,
    Unavailable,
    Disconnected,
    Prepare,
    Config,
    NeedAuth,
    IpConfig,
    IpCheck,
    Secondaries,
    Activated,
    Deactivating,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStateReason {
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Ethernet,
    Wifi,
    Bond,
    Bridge,
    Vlan,
    Team,
    Infiniband,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Unknown,
    Activating,
    Activated,
    Deactivating,
    Deactivated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStateReason {
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpMethod {
    Auto,
    Manual,
    LinkLocal,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityType {
    Open,
    Wep,
    WpaPsk { version: WpaVersion },
    WpaEnterprise { version: WpaVersion },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WpaVersion {
    Wpa1,
    Wpa2,
    Wpa3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiMode {
    Adhoc,
    Infra,
    Ap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiBand {
    A,
    Bg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BondMode {
    RoundRobin,
    ActiveBackup,
    Xor,
    Broadcast,
    Lacp,
    Tlb,
    Alb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NmState {
    Unknown,
    Asleep,
    Disconnected,
    Disconnecting,
    Connecting,
    ConnectedLocal,
    ConnectedSite,
    ConnectedGlobal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityState {
    Unknown,
    None,
    Portal,
    Limited,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeteredState {
    Unknown,
    Yes,
    No,
    GuessYes,
    GuessNo,
}

// =================== Structs ===================

#[derive(Debug, Clone)]
pub struct Device {
    pub interface: String,
    pub ip_interface: Option<String>,
    pub device_type: DeviceType,
    pub state: DeviceState,
    pub state_reason: Option<DeviceStateReason>,
    pub managed: bool,
    pub autoconnect: bool,
    pub mac_address: String,
    pub mtu: u32,
    pub driver: String,
    pub driver_version: String,
}

#[derive(Debug, Clone)]
pub struct AccessPoint {
    pub ssid: Vec<u8>,
    pub bssid: String,
    pub strength: u8,
    pub frequency: u32,
    pub channel: u32,
    pub bandwidth: u32,
    pub max_bitrate: u32,
    pub security: SecurityType,
    pub wps: bool,
    pub mode: WifiMode,
    pub last_seen: u32,
}

#[derive(Debug, Clone)]
pub struct ConnectionProfile {
    pub uuid: Uuid,
    pub name: String,
    pub connection_type: ConnectionType,
    pub autoconnect: bool,
    pub interface_name: Option<String>,
    pub last_used: Option<SystemTime>,
    pub unsaved: bool,
    pub filename: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActiveConnection {
    pub id: String,
    pub profile_uuid: Uuid,
    pub name: String,
    pub connection_type: ConnectionType,
    pub interface: String,
    pub state: ConnectionState,
    pub state_flags: u32,
    pub default: bool,
    pub default6: bool,
    pub vpn: bool,
    pub ip4_config: Option<IpConfig>,
    pub ip6_config: Option<IpConfig>,
    pub devices: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IpConfig {
    pub method: IpMethod,
    pub addresses: Vec<IpAddressWithPrefix>,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
    pub search_domains: Vec<String>,
    pub routes: Vec<IpRoute>,
}

#[derive(Debug, Clone)]
pub struct IpAddressWithPrefix {
    pub address: IpAddr,
    pub prefix: u8,
}

#[derive(Debug, Clone)]
pub struct IpRoute {
    pub destination: IpAddr,
    pub prefix: u8,
    pub next_hop: Option<IpAddr>,
    pub metric: u32,
}

#[derive(Debug, Clone)]
pub struct DeviceStats {
    pub device: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub signal_dbm: i32,
    pub tx_rate: u32,
    pub rx_rate: u32,
}

#[derive(Debug, Clone)]
pub struct GlobalState {
    pub version: String,
    pub state: NmState,
    pub connectivity: ConnectivityState,
    pub networking_enabled: bool,
    pub wireless_enabled: bool,
    pub wireless_hw_enabled: bool,
    pub wwan_enabled: bool,
    pub wwan_hw_enabled: bool,
    pub hostname: String,
    pub metered: MeteredState,
}

// =================== Connection Settings ===================

#[derive(Debug, Clone)]
pub struct ConnectionSettings {
    pub common: CommonSettings,
    pub connection_type: ConnectionTypeSettings,
    pub ipv4: IpSettings,
    pub ipv6: IpSettings,
}

#[derive(Debug, Clone)]
pub struct CommonSettings {
    pub name: String,
    pub uuid: Uuid,
    pub autoconnect: bool,
    pub interface_name: Option<String>,
    pub cloned_mac: Option<String>,
    pub mtu: Option<u32>,
    pub metered: Option<MeteredState>,
    pub zone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IpSettings {
    pub method: IpMethod,
    pub addresses: Vec<IpAddressWithPrefix>,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
    pub search_domains: Vec<String>,
    pub routes: Vec<IpRoute>,
    pub never_default: bool,
    pub ignore_auto_dns: bool,
}

#[derive(Debug, Clone)]
pub enum ConnectionTypeSettings {
    Ethernet {
        speed: Option<u32>,
        duplex: Option<String>,
        auto_negotiate: Option<bool>,
        wake_on_lan: Option<u32>,
        mac_address: Option<String>,
    },
    Wifi {
        ssid: Vec<u8>,
        mode: WifiMode,
        bssid: Option<String>,
        band: Option<WifiBand>,
        channel: Option<u32>,
        security: WifiSecuritySettings,
        hidden: bool,
        powersave: Option<u32>,
    },
    Bond {
        mode: BondMode,
        slaves: Vec<Uuid>,
    },
    Bridge {
        stp: bool,
        priority: u16,
        forward_delay: u16,
        hello_time: u16,
        max_age: u16,
        slaves: Vec<Uuid>,
    },
    Vlan {
        parent: String,
        id: u16,
        flags: u32,
    },
    Team {
        config: String,
        slaves: Vec<Uuid>,
    },
    Infiniband {
        transport_mode: String,
        p_key: Option<i32>,
        parent: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum WifiSecuritySettings {
    None,
    Wep { key: String },
    WpaPsk { version: WpaVersion, password: String },
    WpaEnterprise { version: WpaVersion, eap: EapSettings },
}

#[derive(Debug, Clone)]
pub struct EapSettings {
    pub identity: Option<String>,
    pub phase2_auth: Option<String>,
}

pub type Secrets = HashMap<String, String>;

// =================== Subscription Events ===================

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Added(Device),
    Removed { interface: String },
    StateChanged {
        interface: String,
        old: DeviceState,
        new: DeviceState,
        reason: DeviceStateReason,
    },
    ManagedChanged { interface: String, managed: bool },
    AutoconnectChanged { interface: String, autoconnect: bool },
}

#[derive(Debug, Clone)]
pub enum ApEvent {
    Added { interface: String, ap: AccessPoint },
    Removed { interface: String, bssid: String },
    ScanDone { interface: String },
}

#[derive(Debug, Clone)]
pub enum ProfileEvent {
    Added(ConnectionProfile),
    Updated { uuid: Uuid },
    Removed { uuid: Uuid },
}

#[derive(Debug, Clone)]
pub enum ActiveEvent {
    StateChanged {
        active_id: String,
        old: ConnectionState,
        new: ConnectionState,
        reason: ConnectionStateReason,
    },
}