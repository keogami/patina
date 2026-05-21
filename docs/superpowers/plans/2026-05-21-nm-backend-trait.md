# NetworkManager Backend Trait — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a `NetworkManagerBackend` async trait with RPITIT generics, a full mock implementation behind a `mock` feature flag, and wire it into the existing Application/ HomeData generic chain.

**Architecture:** Monolithic trait with grouped sections (Discovery, Connection Control, Configuration, Global State, Monitoring, Subscriptions). Mock holds state in `Arc<tokio::sync::Mutex<MockState>>`. Application and Pane become generic over `B: NetworkManagerBackend`. `enum_dispatch` on generic `Pane<B>` is assumed to work; if not, fallback to manual `Component` impl.

**Tech Stack:** Rust 2024 edition, tokio (existing), anyhow (existing), uuid (new), ratatui (existing), enum_dispatch (existing)

---

## File Map

| File | Responsibility |
|------|-------------|
| `Cargo.toml` | Add `uuid` dep, `mock` feature, tokio `macros` feature |
| `src/main.rs` | Add `mod backend;`, conditionally create `MockBackend` |
| `src/backend/types.rs` | All backend data types, enums, structs, event types |
| `src/backend/mod.rs` | `NetworkManagerBackend` trait definition, re-exports |
| `src/backend/mock.rs` | `MockBackend` state, constructor, and all trait method impls |
| `src/application/mod.rs` | Make `Application` and `Pane` generic over `Backend` |
| `src/application/home.rs` | Make `HomeData` generic, accept backend, spawn load task |
| `src/application/component.rs` | No changes (trait signature unchanged) |

---

## Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add dependencies and feature flag**

  Append to `Cargo.toml`:

  ```toml
  [features]
  mock = []

  [dependencies]
  uuid = "1"
  ```

  Update the existing `tokio` dependency to add `"macros"` (for `#[tokio::test]`):

  ```toml
  tokio = { version = "1", features = ["rt-multi-thread", "time", "macros"] }
  ```

- [ ] **Step 2: Verify compilation still works**

  Run: `cargo check`
  Expected: PASS (no new code yet, just dependency changes)

- [ ] **Step 3: Commit**

  ```bash
  git add Cargo.toml Cargo.lock
  git commit -m "deps: add uuid, mock feature, tokio macros"
  ```

---

## Task 2: Create All Backend Types

**Files:**
- Create: `src/backend/types.rs`
- Modify: `src/main.rs` (add `mod backend;`)

- [ ] **Step 1: Write `src/backend/types.rs`**

  ```rust
  use std::collections::HashMap;
  use std::net::IpAddr;
  use std::time::{Duration, SystemTime};
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
      LACP,
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
  ```

- [ ] **Step 2: Register the module in `src/main.rs`**

  Add after `mod application;`:

  ```rust
  mod backend;
  ```

- [ ] **Step 3: Verify compilation**

  Run: `cargo check`
  Expected: PASS (types module compiles, nothing uses it yet)

- [ ] **Step 4: Commit**

  ```bash
  git add src/backend/types.rs src/main.rs
  git commit -m "feat: add backend data types"
  ```

---

## Task 3: Define the `NetworkManagerBackend` Trait

**Files:**
- Create: `src/backend/mod.rs`

- [ ] **Step 1: Write `src/backend/mod.rs`**

  ```rust
  use std::collections::HashMap;
  use std::future::Future;

  use anyhow::Result;
  use tokio::sync::mpsc;
  use uuid::Uuid;

  pub mod types;
  pub use types::*;

  #[cfg(feature = "mock")]
  pub mod mock;

  pub trait NetworkManagerBackend: Clone + Send + Sync + 'static {
      // Discovery
      fn get_all_devices(&self) -> impl Future<Output = Result<Vec<Device>>> + Send;
      fn get_device_by_interface(
          &self,
          interface: &str,
      ) -> impl Future<Output = Result<Option<Device>>> + Send;
      fn get_access_points(
          &self,
          interface: &str,
      ) -> impl Future<Output = Result<Vec<AccessPoint>>> + Send;
      fn request_scan(&self, interface: &str) -> impl Future<Output = Result<()>> + Send;

      // Connection Control
      fn list_connections(&self) -> impl Future<Output = Result<Vec<ConnectionProfile>>> + Send;
      fn get_active_connections(&self) -> impl Future<Output = Result<Vec<ActiveConnection>>> + Send;
      fn get_connection_by_uuid(
          &self,
          uuid: &Uuid,
      ) -> impl Future<Output = Result<ConnectionProfile>> + Send;
      fn activate_connection(
          &self,
          uuid: &Uuid,
          interface: Option<&str>,
      ) -> impl Future<Output = Result<ActiveConnection>> + Send;
      fn deactivate_connection(
          &self,
          active_id: &str,
      ) -> impl Future<Output = Result<()>> + Send;
      fn disconnect_device(
          &self,
          interface: &str,
      ) -> impl Future<Output = Result<()>> + Send;
      fn add_and_activate_connection(
          &self,
          settings: &ConnectionSettings,
          interface: Option<&str>,
      ) -> impl Future<Output = Result<ActiveConnection>> + Send;
      fn get_secrets(
          &self,
          uuid: &Uuid,
          setting_name: &str,
      ) -> impl Future<Output = Result<Secrets>> + Send;

      // Configuration (CRUD)
      fn get_settings(
          &self,
          uuid: &Uuid,
      ) -> impl Future<Output = Result<ConnectionSettings>> + Send;
      fn update_connection(
          &self,
          uuid: &Uuid,
          settings: &ConnectionSettings,
      ) -> impl Future<Output = Result<()>> + Send;
      fn add_connection(
          &self,
          settings: &ConnectionSettings,
      ) -> impl Future<Output = Result<ConnectionProfile>> + Send;
      fn delete_connection(&self, uuid: &Uuid) -> impl Future<Output = Result<()>> + Send;

      // Global State
      fn get_global_state(&self) -> impl Future<Output = Result<GlobalState>> + Send;
      fn set_networking_enabled(
          &self,
          enabled: bool,
      ) -> impl Future<Output = Result<()>> + Send;
      fn set_wireless_enabled(
          &self,
          enabled: bool,
      ) -> impl Future<Output = Result<()>> + Send;
      fn set_wwan_enabled(
          &self,
          enabled: bool,
      ) -> impl Future<Output = Result<()>> + Send;
      fn save_hostname(&self, hostname: &str) -> impl Future<Output = Result<()>> + Send;

      // Device Settings
      fn set_device_managed(
          &self,
          interface: &str,
          managed: bool,
      ) -> impl Future<Output = Result<()>> + Send;
      fn set_device_autoconnect(
          &self,
          interface: &str,
          autoconnect: bool,
      ) -> impl Future<Output = Result<()>> + Send;

      // Monitoring
      fn get_device_stats(
          &self,
          interface: &str,
      ) -> impl Future<Output = Result<DeviceStats>> + Send;

      // Subscriptions
      fn subscribe_device_events(
          &self,
      ) -> impl Future<Output = Result<mpsc::UnboundedReceiver<DeviceEvent>>> + Send;
      fn subscribe_ap_events(
          &self,
          interface: &str,
      ) -> impl Future<Output = Result<mpsc::UnboundedReceiver<ApEvent>>> + Send;
      fn subscribe_profile_events(
          &self,
      ) -> impl Future<Output = Result<mpsc::UnboundedReceiver<ProfileEvent>>> + Send;
      fn subscribe_active_events(
          &self,
      ) -> impl Future<Output = Result<mpsc::UnboundedReceiver<ActiveEvent>>> + Send;
  }
  ```

- [ ] **Step 2: Verify compilation**

  Run: `cargo check`
  Expected: PASS (trait compiles but no impl yet)

- [ ] **Step 3: Commit**

  ```bash
  git add src/backend/mod.rs
  git commit -m "feat: define NetworkManagerBackend trait"
  ```

---

## Task 4: Implement MockBackend — Constructor and Discovery

**Files:**
- Create: `src/backend/mock.rs`

- [ ] **Step 1: Write failing test**

  Create `src/backend/mock.rs` with the test block first:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[tokio::test]
      async fn mock_lists_default_devices() {
          let backend = MockBackend::new();
          let devices = backend.get_all_devices().await.unwrap();
          assert_eq!(devices.len(), 2);
          assert!(devices.iter().any(|d| d.interface == "wlan0"));
          assert!(devices.iter().any(|d| d.interface == "eth0"));
      }

      #[tokio::test]
      async fn mock_lists_access_points_for_wifi_device() {
          let backend = MockBackend::new();
          let aps = backend.get_access_points("wlan0").await.unwrap();
          assert!(!aps.is_empty());
          assert!(aps.iter().any(|ap| ap.ssid == b"Overcast-5G"));
      }
  }
  ```

- [ ] **Step 2: Run test to verify failure**

  Run: `cargo test --features mock -- mock_lists`
  Expected: FAIL — `MockBackend` not found

- [ ] **Step 3: Write MockBackend constructor and discovery methods**

  Append the implementation to `src/backend/mock.rs` (before the `#[cfg(test)]` block):

  ```rust
  use std::collections::HashMap;
  use std::sync::Arc;
  use std::time::{Duration, SystemTime};

  use anyhow::{Context, Result};
  use tokio::sync::{Mutex, mpsc};
  use uuid::Uuid;

  use crate::backend::types::*;

  #[derive(Clone)]
  pub struct MockBackend(Arc<Mutex<MockState>>);

  struct MockState {
      devices: Vec<Device>,
      access_points: HashMap<String, Vec<AccessPoint>>,
      saved_connections: Vec<ConnectionProfile>,
      connection_settings: HashMap<Uuid, ConnectionSettings>,
      active_connections: Vec<ActiveConnection>,
      next_active_id: u64,
      scanning: HashMap<String, bool>,
      device_stats: HashMap<String, DeviceStats>,
      device_event_txs: Vec<mpsc::UnboundedSender<DeviceEvent>>,
      ap_event_txs: HashMap<String, Vec<mpsc::UnboundedSender<ApEvent>>>,
      profile_event_txs: Vec<mpsc::UnboundedSender<ProfileEvent>>,
      active_event_txs: Vec<mpsc::UnboundedSender<ActiveEvent>>,
      global_state: GlobalState,
      hostname: String,
  }

  impl MockBackend {
      pub fn new() -> Self {
          let mut access_points = HashMap::new();
          access_points.insert("wlan0".into(), vec![
              AccessPoint { ssid: b"Overcast-5G".to_vec(), bssid: "A4:2B:B0:FE:31:9C".into(), strength: 82, frequency: 5220, channel: 44, bandwidth: 80, max_bitrate: 650_000, security: SecurityType::WpaPsk { version: WpaVersion::Wpa2 }, wps: false, mode: WifiMode::Infra, last_seen: 0 },
              AccessPoint { ssid: b"Acme-Corp".to_vec(), bssid: "88:3D:24:17:02:A1".into(), strength: 71, frequency: 5750, channel: 149, bandwidth: 80, max_bitrate: 867_000, security: SecurityType::WpaEnterprise { version: WpaVersion::Wpa2 }, wps: false, mode: WifiMode::Infra, last_seen: 5 },
              AccessPoint { ssid: b"FiberLink_9A82".to_vec(), bssid: "1C:83:41:9A:82:00".into(), strength: 58, frequency: 6130, channel: 37, bandwidth: 160, max_bitrate: 1_201_000, security: SecurityType::WpaPsk { version: WpaVersion::Wpa3 }, wps: false, mode: WifiMode::Infra, last_seen: 12 },
              AccessPoint { ssid: b"xfinitywifi".to_vec(), bssid: "0C:54:A5:88:2B:77".into(), strength: 42, frequency: 2412, channel: 1, bandwidth: 20, max_bitrate: 150_000, security: SecurityType::Open, wps: false, mode: WifiMode::Infra, last_seen: 30 },
              AccessPoint { ssid: b"TP-Link_5544".to_vec(), bssid: "E4:95:6E:55:44:A0".into(), strength: 28, frequency: 2437, channel: 6, bandwidth: 20, max_bitrate: 300_000, security: SecurityType::WpaPsk { version: WpaVersion::Wpa2 }, wps: false, mode: WifiMode::Infra, last_seen: 60 },
          ]);

          let wlan0_uuid = Uuid::new_v4();
          let eth0_uuid = Uuid::new_v4();
          let saved_wifi_uuid = Uuid::new_v4();

          let saved_connections = vec![
              ConnectionProfile { uuid: wlan0_uuid, name: "Overcast-5G".into(), connection_type: ConnectionType::Wifi, autoconnect: true, interface_name: Some("wlan0".into()), last_used: Some(SystemTime::now() - Duration::from_secs(300)), unsaved: false, filename: Some("/etc/NetworkManager/system-connections/Overcast-5G.nmconnection".into()) },
              ConnectionProfile { uuid: eth0_uuid, name: "Wired".into(), connection_type: ConnectionType::Ethernet, autoconnect: true, interface_name: Some("eth0".into()), last_used: Some(SystemTime::now() - Duration::from_secs(86400)), unsaved: false, filename: Some("/etc/NetworkManager/system-connections/Wired.nmconnection".into()) },
              ConnectionProfile { uuid: saved_wifi_uuid, name: "CoffeeShop".into(), connection_type: ConnectionType::Wifi, autoconnect: false, interface_name: None, last_used: Some(SystemTime::now() - Duration::from_secs(604800)), unsaved: false, filename: Some("/etc/NetworkManager/system-connections/CoffeeShop.nmconnection".into()) },
          ];

          let active_connections = vec![
              ActiveConnection { id: "active-1".into(), profile_uuid: wlan0_uuid, name: "Overcast-5G".into(), connection_type: ConnectionType::Wifi, interface: "wlan0".into(), state: ConnectionState::Activated, state_flags: 0, default: true, default6: true, vpn: false, ip4_config: Some(IpConfig { method: IpMethod::Auto, addresses: vec![IpAddressWithPrefix { address: "10.0.0.147".parse().unwrap(), prefix: 24 }], gateway: Some("10.0.0.1".into()), dns: vec!["10.0.0.1".into()], search_domains: vec![], routes: vec![] }), ip6_config: Some(IpConfig { method: IpMethod::Auto, addresses: vec![IpAddressWithPrefix { address: "fe80::1".parse().unwrap(), prefix: 64 }], gateway: None, dns: vec![], search_domains: vec![], routes: vec![] }), devices: vec!["wlan0".into()] },
              ActiveConnection { id: "active-2".into(), profile_uuid: eth0_uuid, name: "Wired".into(), connection_type: ConnectionType::Ethernet, interface: "eth0".into(), state: ConnectionState::Activated, state_flags: 0, default: false, default6: false, vpn: false, ip4_config: Some(IpConfig { method: IpMethod::Auto, addresses: vec![IpAddressWithPrefix { address: "192.168.1.42".parse().unwrap(), prefix: 24 }], gateway: Some("192.168.1.1".into()), dns: vec!["192.168.1.1".into()], search_domains: vec![], routes: vec![] }), ip6_config: None, devices: vec!["eth0".into()] },
          ];

          let mut connection_settings = HashMap::new();
          connection_settings.insert(wlan0_uuid, ConnectionSettings {
              common: CommonSettings { name: "Overcast-5G".into(), uuid: wlan0_uuid, autoconnect: true, interface_name: Some("wlan0".into()), cloned_mac: None, mtu: None, metered: None, zone: None },
              connection_type: ConnectionTypeSettings::Wifi { ssid: b"Overcast-5G".to_vec(), mode: WifiMode::Infra, bssid: None, band: None, channel: None, security: WifiSecuritySettings::WpaPsk { version: WpaVersion::Wpa2, password: String::new() }, hidden: false, powersave: None },
              ipv4: IpSettings { method: IpMethod::Auto, addresses: vec![], gateway: None, dns: vec![], search_domains: vec![], routes: vec![], never_default: false, ignore_auto_dns: false },
              ipv6: IpSettings { method: IpMethod::Auto, addresses: vec![], gateway: None, dns: vec![], search_domains: vec![], routes: vec![], never_default: false, ignore_auto_dns: false },
          });

          let mut device_stats = HashMap::new();
          device_stats.insert("wlan0".into(), DeviceStats { device: "wlan0".into(), rx_bytes: 1_000_000, tx_bytes: 500_000, rx_packets: 10_000, tx_packets: 5_000, signal_dbm: -45, tx_rate: 650_000, rx_rate: 650_000 });
          device_stats.insert("eth0".into(), DeviceStats { device: "eth0".into(), rx_bytes: 10_000_000, tx_bytes: 5_000_000, rx_packets: 100_000, tx_packets: 50_000, signal_dbm: 0, tx_rate: 1_000_000, rx_rate: 1_000_000 });

          let global_state = GlobalState {
              version: "1.56.0".into(),
              state: NmState::ConnectedGlobal,
              connectivity: ConnectivityState::Full,
              networking_enabled: true,
              wireless_enabled: true,
              wireless_hw_enabled: true,
              wwan_enabled: false,
              wwan_hw_enabled: false,
              hostname: "patina-dev".into(),
              metered: MeteredState::No,
          };

          let state = MockState {
              devices: vec![
                  Device { interface: "wlan0".into(), ip_interface: Some("wlan0".into()), device_type: DeviceType::Wifi, state: DeviceState::Activated, state_reason: None, managed: true, autoconnect: true, mac_address: "A4:2B:B0:FE:31:9C".into(), mtu: 1500, driver: "iwlwifi".into(), driver_version: "0.1".into() },
                  Device { interface: "eth0".into(), ip_interface: Some("eth0".into()), device_type: DeviceType::Ethernet, state: DeviceState::Activated, state_reason: None, managed: true, autoconnect: true, mac_address: "00:1A:2B:3C:4D:5E".into(), mtu: 1500, driver: "e1000e".into(), driver_version: "0.1".into() },
              ],
              access_points,
              saved_connections,
              connection_settings,
              active_connections,
              next_active_id: 3,
              scanning: HashMap::new(),
              device_stats,
              device_event_txs: Vec::new(),
              ap_event_txs: HashMap::new(),
              profile_event_txs: Vec::new(),
              active_event_txs: Vec::new(),
              global_state,
              hostname: "patina-dev".into(),
          };

          Self(Arc::new(Mutex::new(state)))
      }
  }

  impl NetworkManagerBackend for MockBackend {
      fn get_all_devices(&self) -> impl Future<Output = Result<Vec<Device>>> + Send {
          let backend = self.clone();
          async move {
              let state = backend.0.lock().await;
              Ok(state.devices.clone())
          }
      }

      fn get_device_by_interface(
          &self,
          interface: &str,
      ) -> impl Future<Output = Result<Option<Device>>> + Send {
          let backend = self.clone();
          let interface = interface.to_string();
          async move {
              let state = backend.0.lock().await;
              Ok(state.devices.iter().find(|d| d.interface == interface).cloned())
          }
      }

      fn get_access_points(
          &self,
          interface: &str,
      ) -> impl Future<Output = Result<Vec<AccessPoint>>> + Send {
          let backend = self.clone();
          let interface = interface.to_string();
          async move {
              let state = backend.0.lock().await;
              Ok(state.access_points.get(&interface).cloned().unwrap_or_default())
          }
      }

      fn request_scan(&self, interface: &str) -> impl Future<Output = Result<()>> + Send {
          let backend = self.clone();
          let interface = interface.to_string();
          async move {
              {
                  let mut state = backend.0.lock().await;
                  state.scanning.insert(interface.clone(), true);
              }
              tokio::time::sleep(Duration::from_millis(500)).await;
              {
                  let mut state = backend.0.lock().await;
                  state.scanning.insert(interface.clone(), false);
                  // Slightly randomize strengths
                  if let Some(aps) = state.access_points.get_mut(&interface) {
                      for ap in aps.iter_mut() {
                          let delta = (rand::random::<i8>() % 10) as i16;
                          ap.strength = (ap.strength as i16 + delta).clamp(0, 100) as u8;
                      }
                  }
                  // Emit ScanDone to AP subscribers for this interface
                  if let Some(txs) = state.ap_event_txs.get(&interface) {
                      let event = ApEvent::ScanDone { interface: interface.clone() };
                      for tx in txs.iter() {
                          let _ = tx.send(event.clone());
                      }
                  }
              }
              Ok(())
          }
      }

      // Stub remaining trait methods — will be filled in Tasks 5–7
      fn list_connections(&self) -> impl Future<Output = Result<Vec<ConnectionProfile>>> + Send {
          let backend = self.clone();
          async move {
              let state = backend.0.lock().await;
              Ok(state.saved_connections.clone())
          }
      }

      fn get_active_connections(&self) -> impl Future<Output = Result<Vec<ActiveConnection>>> + Send {
          let backend = self.clone();
          async move {
              let state = backend.0.lock().await;
              Ok(state.active_connections.clone())
          }
      }

      fn get_connection_by_uuid(&self, _uuid: &Uuid) -> impl Future<Output = Result<ConnectionProfile>> + Send {
          async move { anyhow::bail!("not yet implemented") }
      }
      fn activate_connection(&self, _uuid: &Uuid, _interface: Option<&str>) -> impl Future<Output = Result<ActiveConnection>> + Send {
          async move { anyhow::bail!("not yet implemented") }
      }
      fn deactivate_connection(&self, _active_id: &str) -> impl Future<Output = Result<()>> + Send {
          async move { anyhow::bail!("not yet implemented") }
      }
      fn disconnect_device(&self, _interface: &str) -> impl Future<Output = Result<()>> + Send {
          async move { anyhow::bail!("not yet implemented") }
      }
      fn add_and_activate_connection(&self, _settings: &ConnectionSettings, _interface: Option<&str>) -> impl Future<Output = Result<ActiveConnection>> + Send {
          async move { anyhow::bail!("not yet implemented") }
      }
      fn get_secrets(&self, _uuid: &Uuid, _setting_name: &str) -> impl Future<Output = Result<Secrets>> + Send {
          async move { Ok(Secrets::new()) }
      }
      fn get_settings(&self, _uuid: &Uuid) -> impl Future<Output = Result<ConnectionSettings>> + Send {
          async move { anyhow::bail!("not yet implemented") }
      }
      fn update_connection(&self, _uuid: &Uuid, _settings: &ConnectionSettings) -> impl Future<Output = Result<()>> + Send {
          async move { anyhow::bail!("not yet implemented") }
      }
      fn add_connection(&self, _settings: &ConnectionSettings) -> impl Future<Output = Result<ConnectionProfile>> + Send {
          async move { anyhow::bail!("not yet implemented") }
      }
      fn delete_connection(&self, _uuid: &Uuid) -> impl Future<Output = Result<()>> + Send {
          async move { anyhow::bail!("not yet implemented") }
      }
      fn get_global_state(&self) -> impl Future<Output = Result<GlobalState>> + Send {
          let backend = self.clone();
          async move {
              let state = backend.0.lock().await;
              Ok(state.global_state.clone())
          }
      }
      fn set_networking_enabled(&self, _enabled: bool) -> impl Future<Output = Result<()>> + Send {
          async move { Ok(()) }
      }
      fn set_wireless_enabled(&self, _enabled: bool) -> impl Future<Output = Result<()>> + Send {
          async move { Ok(()) }
      }
      fn set_wwan_enabled(&self, _enabled: bool) -> impl Future<Output = Result<()>> + Send {
          async move { Ok(()) }
      }
      fn save_hostname(&self, hostname: &str) -> impl Future<Output = Result<()>> + Send {
          let backend = self.clone();
          let hostname = hostname.to_string();
          async move {
              let mut state = backend.0.lock().await;
              state.hostname = hostname;
              Ok(())
          }
      }
      fn set_device_managed(&self, _interface: &str, _managed: bool) -> impl Future<Output = Result<()>> + Send {
          async move { Ok(()) }
      }
      fn set_device_autoconnect(&self, _interface: &str, _autoconnect: bool) -> impl Future<Output = Result<()>> + Send {
          async move { Ok(()) }
      }
      fn get_device_stats(&self, interface: &str) -> impl Future<Output = Result<DeviceStats>> + Send {
          let backend = self.clone();
          let interface = interface.to_string();
          async move {
              let mut state = backend.0.lock().await;
              let stats = state.device_stats.entry(interface.clone()).or_insert_with(|| DeviceStats {
                  device: interface.clone(),
                  rx_bytes: 0,
                  tx_bytes: 0,
                  rx_packets: 0,
                  tx_packets: 0,
                  signal_dbm: -50,
                  tx_rate: 0,
                  rx_rate: 0,
              });
              stats.rx_bytes += 1_000;
              stats.tx_bytes += 500;
              stats.rx_packets += 10;
              stats.tx_packets += 5;
              Ok(stats.clone())
          }
      }
      fn subscribe_device_events(&self) -> impl Future<Output = Result<mpsc::UnboundedReceiver<DeviceEvent>>> + Send {
          let backend = self.clone();
          async move {
              let (tx, rx) = mpsc::unbounded_channel();
              let mut state = backend.0.lock().await;
              state.device_event_txs.push(tx);
              Ok(rx)
          }
      }
      fn subscribe_ap_events(&self, interface: &str) -> impl Future<Output = Result<mpsc::UnboundedReceiver<ApEvent>>> + Send {
          let backend = self.clone();
          let interface = interface.to_string();
          async move {
              let (tx, rx) = mpsc::unbounded_channel();
              let mut state = backend.0.lock().await;
              state.ap_event_txs.entry(interface).or_default().push(tx);
              Ok(rx)
          }
      }
      fn subscribe_profile_events(&self) -> impl Future<Output = Result<mpsc::UnboundedReceiver<ProfileEvent>>> + Send {
          let backend = self.clone();
          async move {
              let (tx, rx) = mpsc::unbounded_channel();
              let mut state = backend.0.lock().await;
              state.profile_event_txs.push(tx);
              Ok(rx)
          }
      }
      fn subscribe_active_events(&self) -> impl Future<Output = Result<mpsc::UnboundedReceiver<ActiveEvent>>> + Send {
          let backend = self.clone();
          async move {
              let (tx, rx) = mpsc::unbounded_channel();
              let mut state = backend.0.lock().await;
              state.active_event_txs.push(tx);
              Ok(rx)
          }
      }
  }
  ```

  **Note:** We need the `rand` crate for `request_scan` strength randomization. Add it to `Cargo.toml` as well:
  ```toml
  rand = "0.8"
  ```

- [ ] **Step 4: Run tests to verify discovery passes**

  Run: `cargo test --features mock -- mock_lists`
  Expected: PASS

- [ ] **Step 5: Commit**

  ```bash
  git add Cargo.toml src/backend/mock.rs
  git commit -m "feat: add MockBackend with discovery and stubs"
  ```

---

## Task 5: Implement MockBackend Connection Control

**Files:**
- Modify: `src/backend/mock.rs`

- [ ] **Step 1: Write failing tests**

  Add to the `#[cfg(test)]` block:

  ```rust
  #[tokio::test]
  async fn mock_activates_connection() {
      let backend = MockBackend::new();
      let conns = backend.list_connections().await.unwrap();
      let target = conns.iter().find(|c| c.name == "CoffeeShop").unwrap();

      let active = backend.activate_connection(&target.uuid, Some("wlan0")).await.unwrap();
      assert_eq!(active.name, "CoffeeShop");
      assert_eq!(active.interface, "wlan0");
      assert_eq!(active.state, ConnectionState::Activated);

      let actives = backend.get_active_connections().await.unwrap();
      assert!(actives.iter().any(|a| a.id == active.id));
  }

  #[tokio::test]
  async fn mock_deactivates_connection() {
      let backend = MockBackend::new();
      let actives = backend.get_active_connections().await.unwrap();
      let id = actives[0].id.clone();

      backend.deactivate_connection(&id).await.unwrap();

      let actives = backend.get_active_connections().await.unwrap();
      assert!(!actives.iter().any(|a| a.id == id));
  }

  #[tokio::test]
  async fn mock_get_connection_by_uuid() {
      let backend = MockBackend::new();
      let conns = backend.list_connections().await.unwrap();
      let uuid = conns[0].uuid;

      let found = backend.get_connection_by_uuid(&uuid).await.unwrap();
      assert_eq!(found.uuid, uuid);
  }
  ```

- [ ] **Step 2: Run tests to verify failure**

  Run: `cargo test --features mock -- mock_activates mock_deactivates mock_get_connection`
  Expected: FAIL — "not yet implemented"

- [ ] **Step 3: Replace stub implementations with real ones**

  Replace the stub methods in `MockBackend`'s `NetworkManagerBackend` impl with:

  ```rust
  fn get_connection_by_uuid(&self, uuid: &Uuid) -> impl Future<Output = Result<ConnectionProfile>> + Send {
      let backend = self.clone();
      let uuid = *uuid;
      async move {
          let state = backend.0.lock().await;
          state.saved_connections.iter()
              .find(|c| c.uuid == uuid)
              .cloned()
              .context("Connection not found")
      }
  }

  fn activate_connection(&self, uuid: &Uuid, interface: Option<&str>) -> impl Future<Output = Result<ActiveConnection>> + Send {
      let backend = self.clone();
      let uuid = *uuid;
      let interface = interface.map(|s| s.to_string());
      async move {
          let mut state = backend.0.lock().await;
          let profile = state.saved_connections.iter()
              .find(|c| c.uuid == uuid)
              .cloned()
              .context("Connection not found")?;

          let id = format!("active-{}", state.next_active_id);
          state.next_active_id += 1;

          let interface = interface.unwrap_or_else(|| profile.interface_name.clone().unwrap_or_else(|| "unknown".into()));

          let active = ActiveConnection {
              id: id.clone(),
              profile_uuid: profile.uuid,
              name: profile.name.clone(),
              connection_type: profile.connection_type,
              interface: interface.clone(),
              state: ConnectionState::Activated,
              state_flags: 0,
              default: false,
              default6: false,
              vpn: false,
              ip4_config: Some(IpConfig {
                  method: IpMethod::Auto,
                  addresses: vec![IpAddressWithPrefix { address: "192.168.1.100".parse().unwrap(), prefix: 24 }],
                  gateway: Some("192.168.1.1".into()),
                  dns: vec!["192.168.1.1".into()],
                  search_domains: vec![],
                  routes: vec![],
              }),
              ip6_config: None,
              devices: vec![interface],
          };

          // Emit state change to subscribers
          for tx in state.active_event_txs.iter() {
              let _ = tx.send(ActiveEvent::StateChanged {
                  active_id: id.clone(),
                  old: ConnectionState::Unknown,
                  new: ConnectionState::Activated,
                  reason: ConnectionStateReason::Unknown,
              });
          }

          state.active_connections.push(active.clone());
          Ok(active)
      }
  }

  fn deactivate_connection(&self, active_id: &str) -> impl Future<Output = Result<()>> + Send {
      let backend = self.clone();
      let active_id = active_id.to_string();
      async move {
          let mut state = backend.0.lock().await;
          let idx = state.active_connections.iter()
              .position(|a| a.id == active_id)
              .context("Active connection not found")?;

          let removed = state.active_connections.remove(idx);

          for tx in state.active_event_txs.iter() {
              let _ = tx.send(ActiveEvent::StateChanged {
                  active_id: removed.id.clone(),
                  old: ConnectionState::Activated,
                  new: ConnectionState::Deactivated,
                  reason: ConnectionStateReason::Unknown,
              });
          }

          Ok(())
      }
  }

  fn disconnect_device(&self, interface: &str) -> impl Future<Output = Result<()>> + Send {
      let backend = self.clone();
      let interface = interface.to_string();
      async move {
          let mut state = backend.0.lock().await;
          let to_remove: Vec<String> = state.active_connections.iter()
              .filter(|a| a.interface == interface)
              .map(|a| a.id.clone())
              .collect();
          for id in to_remove {
              if let Some(idx) = state.active_connections.iter().position(|a| a.id == id) {
                  state.active_connections.remove(idx);
              }
          }
          Ok(())
      }
  }

  fn add_and_activate_connection(&self, settings: &ConnectionSettings, interface: Option<&str>) -> impl Future<Output = Result<ActiveConnection>> + Send {
      let backend = self.clone();
      let settings = settings.clone();
      let interface = interface.map(|s| s.to_string());
      async move {
          let profile = backend.add_connection(&settings).await?;
          backend.activate_connection(&profile.uuid, interface.as_deref()).await
      }
  }
  ```

  Note: `get_secrets` stub is fine — returns empty `Secrets`. `list_connections` and `get_active_connections` were already implemented in Task 4.

- [ ] **Step 4: Run tests to verify pass**

  Run: `cargo test --features mock -- mock_activates mock_deactivates mock_get_connection`
  Expected: PASS

- [ ] **Step 5: Commit**

  ```bash
  git add src/backend/mock.rs
  git commit -m "feat: implement MockBackend connection control"
  ```

---

## Task 6: Implement MockBackend Configuration and Global State

**Files:**
- Modify: `src/backend/mock.rs`

- [ ] **Step 1: Write failing tests**

  Add to test block:

  ```rust
  #[tokio::test]
  async fn mock_crud_connection() {
      let backend = MockBackend::new();

      // Add
      let settings = ConnectionSettings {
          common: CommonSettings {
              name: "TestNet".into(),
              uuid: Uuid::new_v4(),
              autoconnect: true,
              interface_name: None,
              cloned_mac: None,
              mtu: None,
              metered: None,
              zone: None,
          },
          connection_type: ConnectionTypeSettings::Wifi {
              ssid: b"TestNet".to_vec(),
              mode: WifiMode::Infra,
              bssid: None,
              band: None,
              channel: None,
              security: WifiSecuritySettings::WpaPsk { version: WpaVersion::Wpa2, password: "secret".into() },
              hidden: false,
              powersave: None,
          },
          ipv4: IpSettings { method: IpMethod::Auto, addresses: vec![], gateway: None, dns: vec![], search_domains: vec![], routes: vec![], never_default: false, ignore_auto_dns: false },
          ipv6: IpSettings { method: IpMethod::Auto, addresses: vec![], gateway: None, dns: vec![], search_domains: vec![], routes: vec![], never_default: false, ignore_auto_dns: false },
      };
      let profile = backend.add_connection(&settings).await.unwrap();
      assert_eq!(profile.name, "TestNet");

      // Read back
      let conns = backend.list_connections().await.unwrap();
      assert!(conns.iter().any(|c| c.uuid == profile.uuid));

      // Get settings
      let fetched = backend.get_settings(&profile.uuid).await.unwrap();
      assert_eq!(fetched.common.name, "TestNet");

      // Update
      let mut updated = fetched.clone();
      updated.common.name = "TestNetUpdated".into();
      backend.update_connection(&profile.uuid, &updated).await.unwrap();

      let fetched2 = backend.get_settings(&profile.uuid).await.unwrap();
      assert_eq!(fetched2.common.name, "TestNetUpdated");

      // Delete
      backend.delete_connection(&profile.uuid).await.unwrap();
      let conns = backend.list_connections().await.unwrap();
      assert!(!conns.iter().any(|c| c.uuid == profile.uuid));
  }

  #[tokio::test]
  async fn mock_global_state() {
      let backend = MockBackend::new();
      let state = backend.get_global_state().await.unwrap();
      assert_eq!(state.hostname, "patina-dev");

      backend.save_hostname("new-host").await.unwrap();
      let state = backend.get_global_state().await.unwrap();
      assert_eq!(state.hostname, "new-host");
  }
  ```

- [ ] **Step 2: Run tests to verify failure**

  Run: `cargo test --features mock -- mock_crud mock_global_state`
  Expected: FAIL — "not yet implemented"

- [ ] **Step 3: Replace stub implementations**

  Replace stubs with:

  ```rust
  fn get_settings(&self, uuid: &Uuid) -> impl Future<Output = Result<ConnectionSettings>> + Send {
      let backend = self.clone();
      let uuid = *uuid;
      async move {
          let state = backend.0.lock().await;
          state.connection_settings.get(&uuid)
              .cloned()
              .context("Settings not found")
      }
  }

  fn update_connection(&self, uuid: &Uuid, settings: &ConnectionSettings) -> impl Future<Output = Result<()>> + Send {
      let backend = self.clone();
      let uuid = *uuid;
      let settings = settings.clone();
      async move {
          let mut state = backend.0.lock().await;
          if !state.connection_settings.contains_key(&uuid) {
              anyhow::bail!("Connection settings not found");
          }
          state.connection_settings.insert(uuid, settings);

          // Update profile name if changed
          if let Some(conn) = state.saved_connections.iter_mut().find(|c| c.uuid == uuid) {
              conn.name = settings.common.name.clone();
              conn.unsaved = true;
          }

          for tx in state.profile_event_txs.iter() {
              let _ = tx.send(ProfileEvent::Updated { uuid });
          }
          Ok(())
      }
  }

  fn add_connection(&self, settings: &ConnectionSettings) -> impl Future<Output = Result<ConnectionProfile>> + Send {
      let backend = self.clone();
      let settings = settings.clone();
      async move {
          let mut state = backend.0.lock().await;
          let uuid = settings.common.uuid;
          let profile = ConnectionProfile {
              uuid,
              name: settings.common.name.clone(),
              connection_type: match &settings.connection_type {
                  ConnectionTypeSettings::Ethernet { .. } => ConnectionType::Ethernet,
                  ConnectionTypeSettings::Wifi { .. } => ConnectionType::Wifi,
                  ConnectionTypeSettings::Bond { .. } => ConnectionType::Bond,
                  ConnectionTypeSettings::Bridge { .. } => ConnectionType::Bridge,
                  ConnectionTypeSettings::Vlan { .. } => ConnectionType::Vlan,
                  ConnectionTypeSettings::Team { .. } => ConnectionType::Team,
                  ConnectionTypeSettings::Infiniband { .. } => ConnectionType::Infiniband,
              },
              autoconnect: settings.common.autoconnect,
              interface_name: settings.common.interface_name.clone(),
              last_used: None,
              unsaved: false,
              filename: None,
          };
          state.connection_settings.insert(uuid, settings);
          state.saved_connections.push(profile.clone());

          for tx in state.profile_event_txs.iter() {
              let _ = tx.send(ProfileEvent::Added(profile.clone()));
          }
          Ok(profile)
      }
  }

  fn delete_connection(&self, uuid: &Uuid) -> impl Future<Output = Result<()>> + Send {
      let backend = self.clone();
      let uuid = *uuid;
      async move {
          let mut state = backend.0.lock().await;
          let idx = state.saved_connections.iter()
              .position(|c| c.uuid == uuid)
              .context("Connection not found")?;
          state.saved_connections.remove(idx);
          state.connection_settings.remove(&uuid);

          for tx in state.profile_event_txs.iter() {
              let _ = tx.send(ProfileEvent::Removed { uuid });
          }
          Ok(())
      }
  }

  fn set_networking_enabled(&self, enabled: bool) -> impl Future<Output = Result<()>> + Send {
      let backend = self.clone();
      async move {
          let mut state = backend.0.lock().await;
          state.global_state.networking_enabled = enabled;
          Ok(())
      }
  }

  fn set_wireless_enabled(&self, enabled: bool) -> impl Future<Output = Result<()>> + Send {
      let backend = self.clone();
      async move {
          let mut state = backend.0.lock().await;
          state.global_state.wireless_enabled = enabled;
          Ok(())
      }
  }

  fn set_wwan_enabled(&self, enabled: bool) -> impl Future<Output = Result<()>> + Send {
      let backend = self.clone();
      async move {
          let mut state = backend.0.lock().await;
          state.global_state.wwan_enabled = enabled;
          Ok(())
      }
  }

  fn set_device_managed(&self, interface: &str, managed: bool) -> impl Future<Output = Result<()>> + Send {
      let backend = self.clone();
      let interface = interface.to_string();
      async move {
          let mut state = backend.0.lock().await;
          if let Some(dev) = state.devices.iter_mut().find(|d| d.interface == interface) {
              dev.managed = managed;
              for tx in state.device_event_txs.iter() {
                  let _ = tx.send(DeviceEvent::ManagedChanged { interface: interface.clone(), managed });
              }
          }
          Ok(())
      }
  }

  fn set_device_autoconnect(&self, interface: &str, autoconnect: bool) -> impl Future<Output = Result<()>> + Send {
      let backend = self.clone();
      let interface = interface.to_string();
      async move {
          let mut state = backend.0.lock().await;
          if let Some(dev) = state.devices.iter_mut().find(|d| d.interface == interface) {
              dev.autoconnect = autoconnect;
              for tx in state.device_event_txs.iter() {
                  let _ = tx.send(DeviceEvent::AutoconnectChanged { interface: interface.clone(), autoconnect });
              }
          }
          Ok(())
      }
  }
  ```

- [ ] **Step 4: Run tests to verify pass**

  Run: `cargo test --features mock -- mock_crud mock_global_state`
  Expected: PASS

- [ ] **Step 5: Commit**

  ```bash
  git add src/backend/mock.rs
  git commit -m "feat: implement MockBackend config, global state, device settings"
  ```

---

## Task 7: Implement MockBackend Subscriptions — Full Test Coverage

**Files:**
- Modify: `src/backend/mock.rs`

- [ ] **Step 1: Write failing tests**

  Add to test block:

  ```rust
  #[tokio::test]
  async fn mock_subscription_receives_scan_done() {
      let backend = MockBackend::new();
      let mut rx = backend.subscribe_ap_events("wlan0").await.unwrap();

      backend.request_scan("wlan0").await.unwrap();

      // Wait up to 1 second for the event
      let event = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
      assert!(event.is_ok());
      assert!(matches!(event.unwrap(), Some(ApEvent::ScanDone { .. })));
  }

  #[tokio::test]
  async fn mock_subscription_receives_device_events() {
      let backend = MockBackend::new();
      let mut rx = backend.subscribe_device_events().await.unwrap();

      backend.set_device_managed("wlan0", false).await.unwrap();

      let event = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
      assert!(event.is_ok());
      assert!(matches!(event.unwrap(), Some(DeviceEvent::ManagedChanged { interface, managed }) if interface == "wlan0" && !managed));
  }
  ```

- [ ] **Step 2: Run tests to verify failure**

  Run: `cargo test --features mock -- mock_subscription`
  Expected: FAIL — if the stubs already returned channels, these might pass. Verify by checking the test output.

  If they already pass because stubs were written in Task 4, skip to Step 4.

- [ ] **Step 3: Fix any missing subscription logic**

  The subscription methods were already implemented in Task 4. If tests fail, check that `set_device_managed` and `request_scan` correctly emit events to all subscribers in the `*_txs` vectors. The Task 4 and Task 6 code already includes `for tx in state.*_event_txs.iter()` loops — ensure they are present.

- [ ] **Step 4: Run tests to verify pass**

  Run: `cargo test --features mock -- mock_subscription`
  Expected: PASS

- [ ] **Step 5: Run full mock backend test suite**

  Run: `cargo test --features mock -- backend::mock`
  Expected: ALL PASS

- [ ] **Step 6: Commit**

  ```bash
  git add src/backend/mock.rs
  git commit -m "test: add MockBackend subscription and integration tests"
  ```

---

## Task 8: Wire Backend into Application

**Files:**
- Modify: `src/application/mod.rs`
- Modify: `src/application/home.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Make Application generic over Backend**

  In `src/application/mod.rs`:

  ```rust
  pub struct Application<B: NetworkManagerBackend> {
      backend: B,
      context: Arc<RichContext>,
      pane: Pane<B>,
      message_tx: Arc<std::sync::mpsc::Sender<Message>>,
      message_rx: Option<std::sync::mpsc::Receiver<Message>>,
  }

  impl<B: NetworkManagerBackend> Application<B> {
      pub fn new(
          backend: B,
          Context { log_file, fps, connection_maxitem }: Context,
      ) -> Self {
          let (message_tx, message_rx) = std::sync::mpsc::channel();
          let message_tx = Arc::new(message_tx);

          let context = RichContext {
              log_file,
              fps,
              message: message_tx.clone(),
              connection_maxitem,
          };

          Self {
              backend: backend.clone(),
              pane: Pane::Home(HomeData::new(backend, &context)),
              context: context.into(),
              message_tx,
              message_rx: Some(message_rx),
          }
      }
      // ... rest of Application impl stays the same
  }

  #[enum_dispatch(Component)]
  pub enum Pane<B: NetworkManagerBackend> {
      Home(HomeData<B>),
  }
  ```

  **Fallback note:** If `enum_dispatch` fails on generic `Pane<B>`, remove the derive and write:
  ```rust
  impl<B: NetworkManagerBackend> Component for Pane<B> {
      fn update(&mut self, ctx: &RichContext, ev: Message) -> Bubble {
          match self {
              Pane::Home(h) => h.update(ctx, ev),
          }
      }
      fn draw(&mut self, ctx: &RichContext, frame: &mut Frame<'_>) {
          match self {
              Pane::Home(h) => h.draw(ctx, frame),
          }
      }
  }
  ```

- [ ] **Step 2: Make HomeData generic and accept backend**

  In `src/application/home.rs`, change `HomeData` to:

  ```rust
  pub struct HomeData<B: NetworkManagerBackend> {
      backend: B,
      loading: Option<Spinner>,
      connected: Connected,
      available: Available,
      selected: Selected,
      connected_scroll_state: ScrollState,
      available_scroll_state: ScrollState,
      available_max_items: Cell<usize>,
      authenticate: Option<authenticate::Authenticate>,
      info: Option<Info>,
  }
  ```

  And `impl<B: NetworkManagerBackend> HomeData<B>` with `new` updated to:

  ```rust
  impl<B: NetworkManagerBackend> HomeData<B> {
      pub fn new(backend: B, ctx: &RichContext) -> Self {
          let loading = Spinner::new(SpinnerType::dot(), ctx.fps);
          let connected = Connected { list: Vec::new() };
          let available = Available { list: Vec::new() };
          let selected = Selected::None;

          let message_tx = ctx.message.clone();
          let backend_clone = backend.clone();
          tokio::spawn(async move {
              // Load from backend instead of hardcoded data
              let devices = backend_clone.get_all_devices().await.unwrap_or_default();
              let active_conns = backend_clone.get_active_connections().await.unwrap_or_default();
              let aps = if let Some(wifi) = devices.iter().find(|d| matches!(d.device_type, DeviceType::Wifi)) {
                  backend_clone.get_access_points(&wifi.interface).await.unwrap_or_default()
              } else {
                  Vec::new()
              };

              // Map backend types to UI types and send messages
              let connected_list: Vec<_> = active_conns.into_iter()
                  .map(|ac| Arc::new(map_active_to_connection_data(ac)))
                  .collect();
              let _ = message_tx.send(Message::LoadConnected(Connected { list: connected_list }));

              let available_list: Vec<_> = aps.into_iter()
                  .map(|ap| Arc::new(map_ap_to_available_details(ap)))
                  .collect();
              let _ = message_tx.send(Message::LoadAvailable(Available { list: available_list }));
              let _ = message_tx.send(Message::FinishLoading);
          });

          Self {
              backend,
              loading,
              connected,
              available,
              selected,
              connected_scroll_state: ScrollState::default(),
              available_scroll_state: ScrollState::default(),
              available_max_items: Cell::new(0),
              authenticate: None,
              info: None,
          }
      }
  }
  ```

  Add helper functions at module level (or in the `home` module):

  ```rust
  fn map_active_to_connection_data(active: ActiveConnection) -> ConnectionData {
      // For now, implement a minimal mapping.
      // In a full implementation, this maps all fields from ActiveConnection + IpConfig.
      // Since the existing UI types are rich, this is a best-effort mapping.
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
          saved: false, // Mock doesn't track saved-by-SSID; override in caller if needed
          signal_history: CircularBuffer::new(),
      }
  }
  ```

  **Note:** The `map_active_to_connection_data` function uses `CircularBuffer` from the existing code. Add `use circular_buffer::CircularBuffer;` to the imports in `home.rs` if not already present.

- [ ] **Step 3: Update main.rs to create backend and pass to Application**

  In `src/main.rs`:

  ```rust
  mod backend;

  #[cfg(feature = "mock")]
  use crate::backend::mock::MockBackend;

  // ... in main():
  let context = AppContext {
      log_file,
      fps: 144,
      connection_maxitem: 5,
  };

  #[cfg(feature = "mock")]
  let application = Application::new(MockBackend::new(), context);

  #[cfg(not(feature = "mock"))]
  compile_error!("Real backend not yet implemented. Build with --features mock.");

  application.run()?;
  ```

- [ ] **Step 4: Remove old inline mock data from HomeData::new**

  The large inline mock arrays (the `let available: Vec<_> = [...]` and `let conns: Vec<_> = [...]` blocks) in the original `HomeData::new` are now replaced by the backend loading task. Delete those old arrays and the old `tokio::spawn` block that created them.

- [ ] **Step 5: Verify compilation with mock feature**

  Run: `cargo check --features mock`
  Expected: PASS

  If `enum_dispatch` complains about generics on `Pane<B>`, switch to the manual `Component` impl shown in Step 1 fallback.

- [ ] **Step 6: Verify compilation without mock feature**

  Run: `cargo check`
  Expected: COMPILE ERROR from the `compile_error!` macro in main.rs — this is intentional.

- [ ] **Step 7: Commit**

  ```bash
  git add src/application/mod.rs src/application/home.rs src/main.rs
  git commit -m "feat: wire generic backend into Application and HomeData"
  ```

---

## Task 9: Full Verification

- [ ] **Step 1: Run all tests**

  Run: `cargo test --features mock`
  Expected: ALL PASS

- [ ] **Step 2: Run clippy**

  Run: `cargo clippy --features mock -- -D warnings`
  Expected: PASS (or fix any warnings)

- [ ] **Step 3: Verify the TUI still starts**

  Run: `cargo run --features mock`
  Expected: TUI opens with mock data loaded from backend (same visual output as before)
  Press `q` to quit.

- [ ] **Step 4: Final commit**

  ```bash
  git add -A
  git commit -m "feat: complete NetworkManager backend trait with MockBackend"
  ```

---

## Plan Self-Review

**1. Spec coverage check:**
- All trait methods from the spec are present in Task 3. ✓
- Mock implements all methods across Tasks 4–7. ✓
- Subscription per-interface model (Device, AP, Profile, Active) matches spec. ✓
- Application wiring replaces hardcoded mock data with backend calls. ✓

**2. Placeholder scan:**
- No "TBD", "TODO", or "implement later" steps. ✓
- All code is concrete and complete. ✓
- `map_active_to_connection_data` is a best-effort mapping — noted as minimal in the step. ✓

**3. Type consistency:**
- `MockBackend` uses `Arc<Mutex<MockState>>` consistently. ✓
- `NetworkManagerBackend` supertrait is `Clone + Send + Sync + 'static`. ✓
- All event types (`DeviceEvent`, `ApEvent`, etc.) match spec definitions from Task 2. ✓

**4. Gap check:**
- `rand` dependency added for `request_scan` strength randomization. ✓
- `tokio::macros` added for `#[tokio::test]`. ✓
- `compile_error!` guard for non-mock builds. ✓
- `enum_dispatch` fallback noted if generics fail. ✓

---

*Plan complete and saved to `docs/superpowers/plans/2026-05-21-nm-backend-trait.md`.*

**Execution options:**

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — Execute tasks in this session using `executing-plans`, batch execution with checkpoints.

Which approach?