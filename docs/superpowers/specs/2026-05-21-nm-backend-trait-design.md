# NetworkManager Backend Trait — Design Spec

> Date: 2026-05-21  
> Scope: nmtui feature parity only (Pillar 1: Discovery & Search; Pillar 3: Configuration basics). Advanced features (VPN, WPA-E, checkpoints, stats monitoring) are out of scope and marked as deferred.

---

## 1. Goals

- Model NetworkManager as an async trait (`NetworkManagerBackend`) with two implementations: `MockBackend` (behind `#[cfg(feature = "mock")]`) and a future `ZbusBackend`.
- Trait must be **stateless**, **`Send + Sync`**, and **cheap to clone** (wraps `Arc<InnerState>`).
- Native Rust 2024 `async fn` is avoided in favor of `impl Future<...> + Send` RPITIT (return-position impl trait in traits) because `async fn` does not automatically imply `Send` on the returned future. This keeps the trait non-object-safe, matching the generic-type-parameter approach.
- No external `async-trait` dependency needed.
- Method surface covers all nmtui features: listing APs, scanning, connecting, listing/activating/deactivating connections, CRUD on connection profiles, device discovery, global state, and D-Bus-aligned subscriptions.

---

## 2. Architecture

```
┌─────────────────┐      ┌──────────────────────────┐      ┌─────────────────┐
│   TUI layer     │◄────►│  Application<Backend>    │◄────►│  Backend impl   │
│  (ratatui)      │      │  (generic type param)    │      │ (Mock / Zbus)   │
└─────────────────┘      └──────────────────────────┘      └─────────────────┘
                                   │                              │
                                   ▼                              ▼
                        ┌─────────────────────┐        ┌─────────────────────┐
                        │ mpsc channel loop   │        │ D-Bus subscriptions │
                        │ (tokio task)        │        │ (per-interface)     │
                        └─────────────────────┘        └─────────────────────┘
```

- The `Application` struct carries a generic `B: NetworkManagerBackend`.
- All backend methods return `anyhow::Result<T>`.
- Background tokio tasks consume subscriptions and translate events into `Message` enums for the main mpsc channel.

---

## 3. Data Types

All types are plain, owned structs — no UI rendering concerns. The UI layer maps these into `ConnectionData`, `AvailableAPDetails`, etc.

### 3.1 Enums

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceType {
    Unknown,
    Ethernet,
    Wifi,
    Bond,
    Bridge,
    Vlan,
    Team,
    Infiniband,
    // Extensible: add new types as needed
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStateReason {
    Unknown,
    // nm-device-state-reason values
    // Deferred: full enumeration for non-mock impls
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionType {
    Ethernet,
    Wifi,
    Bond,
    Bridge,
    Vlan,
    Team,
    Infiniband,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Unknown,
    Activating,
    Activated,
    Deactivating,
    Deactivated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStateReason {
    Unknown,
    // NMActiveConnectionStateReason values
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
pub enum WpaVersion { Wpa1, Wpa2, Wpa3 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WifiMode { Adhoc, Infra, Ap }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WifiBand { A, Bg }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BondMode {
    RoundRobin, ActiveBackup, Xor, Broadcast,
    LACP, Tlb, Alb,
}
```

### 3.2 Structs

```rust
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::SystemTime;
use uuid::Uuid; // new dependency

// Device
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

// AccessPoint
#[derive(Debug, Clone)]
pub struct AccessPoint {
    pub ssid: Vec<u8>,
    pub bssid: String,
    pub strength: u8,          // 0-100
    pub frequency: u32,        // MHz
    pub channel: u32,
    pub bandwidth: u32,        // MHz
    pub max_bitrate: u32,      // Kbps
    pub security: SecurityType,
    pub wps: bool,
    pub mode: WifiMode,
    pub last_seen: u32,        // seconds since CLOCK_BOOTTIME
}

// ConnectionProfile
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

// ActiveConnection
#[derive(Debug, Clone)]
pub struct ActiveConnection {
    pub id: String,            // D-Bus object path or internal ID
    pub profile_uuid: Uuid,
    pub name: String,
    pub connection_type: ConnectionType,
    pub interface: String,
    pub state: ConnectionState,
    pub state_flags: u32,      // NMActivationStateFlags (raw for now)
    pub default: bool,
    pub default6: bool,
    pub vpn: bool,
    pub ip4_config: Option<IpConfig>,
    pub ip6_config: Option<IpConfig>,
    pub devices: Vec<String>,
}

// IpConfig
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

// DeviceStats
#[derive(Debug, Clone)]
pub struct DeviceStats {
    pub device: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub signal_dbm: i32,
    pub tx_rate: u32,     // Kbps
    pub rx_rate: u32,     // Kbps
}

// GlobalState
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

// Connection settings (for add/update)
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
    // Deferred: full WPA-Enterprise support is beyond nmtui parity.
    // Placeholder for identity, phase2, certs, etc.
    pub identity: Option<String>,
    pub phase2_auth: Option<String>,
}

// Secrets
pub type Secrets = HashMap<String, String>;
```

---

## 4. Trait Definition

```rust
use std::future::Future;
use std::collections::HashMap;
use anyhow::Result;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Async NetworkManager backend trait.
///
/// Non-object-safe (RPITIT generics) by design. The Application holds it
/// as a generic type parameter (`Application<B: NetworkManagerBackend>`).
/// Clone is implemented via `Arc<InnerState>` at the impl level.
pub trait NetworkManagerBackend: Clone + Send + Sync + 'static {
    // ══════════════════════════════════════════════════════════
    // Discovery
    // ══════════════════════════════════════════════════════════

    fn get_all_devices(&self) -> impl Future<Output = Result<Vec<Device>>> + Send;
    fn get_device_by_interface(&self, interface: &str) -> impl Future<Output = Result<Option<Device>>> + Send;

    // WiFi-specific
    fn get_access_points(&self, interface: &str) -> impl Future<Output = Result<Vec<AccessPoint>>> + Send;
    fn request_scan(&self, interface: &str) -> impl Future<Output = Result<()>> + Send;

    // ══════════════════════════════════════════════════════════
    // Connection Control
    // ══════════════════════════════════════════════════════════

    fn list_connections(&self) -> impl Future<Output = Result<Vec<ConnectionProfile>>> + Send;
    fn get_active_connections(&self) -> impl Future<Output = Result<Vec<ActiveConnection>>> + Send;
    fn get_connection_by_uuid(&self, uuid: &Uuid) -> impl Future<Output = Result<ConnectionProfile>> + Send;

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

    // ══════════════════════════════════════════════════════════
    // Secrets
    // ══════════════════════════════════════════════════════════

    fn get_secrets(
        &self,
        uuid: &Uuid,
        setting_name: &str,
    ) -> impl Future<Output = Result<Secrets>> + Send;

    // ══════════════════════════════════════════════════════════
    // Configuration (CRUD)
    // ══════════════════════════════════════════════════════════

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

    fn delete_connection(
        &self,
        uuid: &Uuid,
    ) -> impl Future<Output = Result<()>> + Send;

    // ══════════════════════════════════════════════════════════
    // Global State
    // ══════════════════════════════════════════════════════════

    fn get_global_state(&self) -> impl Future<Output = Result<GlobalState>> + Send;
    fn set_networking_enabled(&self, enabled: bool) -> impl Future<Output = Result<()>> + Send;
    fn set_wireless_enabled(&self, enabled: bool) -> impl Future<Output = Result<()>> + Send;
    fn set_wwan_enabled(&self, enabled: bool) -> impl Future<Output = Result<()>> + Send;

    fn save_hostname(&self, hostname: &str) -> impl Future<Output = Result<()>> + Send;

    // ══════════════════════════════════════════════════════════
    // Device Settings
    // ══════════════════════════════════════════════════════════

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

    // ══════════════════════════════════════════════════════════
    // Monitoring
    // ══════════════════════════════════════════════════════════

    fn get_device_stats(
        &self,
        interface: &str,
    ) -> impl Future<Output = Result<DeviceStats>> + Send;

    // ── Per-interface subscriptions (aligned with D-Bus signal groups) ──

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

### 4.1 Subscription Event Types

```rust
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

---

## 5. Error Handling

All methods return `anyhow::Result<T>`. The trait itself does not define an error enum; each implementation is free to use its own error types and wrap them with `.context("...")`.

- **MockBackend**: errors are deterministic (e.g., "device not found", "wrong password for mock network").
- **ZbusBackend** (future): zbus errors, policykit denials, and NM-specific errors all surface through `anyhow`.

---

## 6. Mock Backend

### 6.1 Implementation Plan

- Struct: `MockBackend(Arc<tokio::sync::Mutex<MockState>>)`.
- `MockState` holds:
  - `devices: Vec<Device>`
  - `access_points: Vec<AccessPoint>` (per-device, keyed by interface)
  - `saved_connections: Vec<ConnectionProfile>`
  - `active_connections: Vec<ActiveConnection>`
  - `next_active_id: u64`
  - `scanning: HashMap<String, bool>`
  - `device_stats: HashMap<String, DeviceStats>`
  - `device_event_txs: Vec<mpsc::UnboundedSender<DeviceEvent>>`
  - `ap_event_txs: HashMap<String, Vec<mpsc::UnboundedSender<ApEvent>>>`
  - `profile_event_txs: Vec<mpsc::UnboundedSender<ProfileEvent>>`
  - `active_event_txs: Vec<mpsc::UnboundedSender<ActiveEvent>>`
  - `global_state: GlobalState`
  - `hostname: String`

### 6.2 Behavior

| Method | Behavior |
|--------|----------|
| `get_all_devices` | Returns `devices` clone. |
| `get_access_points` | Returns all mock APs for the given WiFi device. |
| `request_scan` | Sets `scanning[iface] = true`, sleeps 500ms, randomizes AP strengths ±5%, sets `scanning[iface] = false`, emits `ApEvent::ScanDone`. |
| `list_connections` | Returns `saved_connections`. |
| `get_active_connections` | Returns `active_connections`. |
| `activate_connection` | Looks up profile by UUID, creates `ActiveConnection`, adds to `active_connections`, emits `ActiveEvent::StateChanged(Activating → Activated)`. Returns it after 200ms. |
| `deactivate_connection` | Removes from `active_connections`, emits `ActiveEvent::StateChanged → Deactivated`. |
| `disconnect_device` | Deactivates any active connection on the device. |
| `add_and_activate_connection` | Adds to `saved_connections`, calls `activate_connection` logic. |
| `get_secrets` | Returns empty `Secrets` for mock. |
| `get_settings` | Derives `ConnectionSettings` from `ConnectionProfile` + sensible defaults. |
| `update_connection` | Replaces matching profile in `saved_connections`, emits `ProfileEvent::Updated`. |
| `add_connection` | Appends to `saved_connections`, emits `ProfileEvent::Added`, returns new `ConnectionProfile`. |
| `delete_connection` | Removes from `saved_connections`, emits `ProfileEvent::Removed`. |
| `get_global_state` | Returns `global_state`. |
| `set_networking_enabled` / `set_wireless_enabled` / `set_wwan_enabled` | Updates `global_state`, emits `DeviceEvent` for affected devices. |
| `save_hostname` | Updates `hostname` field. |
| `set_device_managed` / `set_device_autoconnect` | Updates device in `devices`, emits `DeviceEvent`. |
| `get_device_stats` | Returns incrementing counters (rx/tx bytes grow on each poll) for the given device. Signal dBm derived from WiFi device's first AP or a fixed value. |
| `subscribe_*` | Creates `(tx, rx)` unbounded channel, stores `tx` in `MockState`, returns `rx`. Drop the receiver to stop receiving (but sender stays in state for simplicity; cleanup is deferred). |

### 6.3 Initial Data

`MockBackend::new()` populates the same data currently hardcoded in `HomeData::new` (the 5 mock APs, 3 mock connections) but translated into backend types. This replaces the inline mock arrays in `home.rs`.

---

## 7. File Layout

```
src/
├── main.rs
├── application/
│   ├── mod.rs
│   ├── component.rs
│   ├── home/
│   ├── theme.rs
│   └── utils.rs
└── backend/
    ├── mod.rs           // trait definition + re-exports
    ├── types.rs         // all data types, enums, event types
    └── mock.rs          // MockBackend (behind #[cfg(feature = "mock")])
```

---

## 8. Dependencies

New additions to `Cargo.toml`:

```toml
[dependencies]
uuid = { version = "1", features = ["v4", "serde"] }

[features]
mock = []
```

Existing tokio features are sufficient: `rt-multi-thread`, `time`, `sync` (already present via `rt-multi-thread` pulling `sync`).

---

## 9. Deferred (Beyond nmtui Parity)

These are explicitly out of scope for this design but the architecture supports them:

| Feature | Why Deferred |
|---------|-------------|
| VPN profiles (`vpn`, `wireguard` settings) | Requires plugin-specific data dictionaries. |
| WPA-Enterprise / 802.1X | Complex EAP settings, certificate handling. |
| Checkpoints (safe rollback) | `CheckpointCreate/Rollback` methods. |
| Secret Agent (on-demand password prompt) | Requires implementing a D-Bus service interface. |
| DNS manager, connectivity check | Global configuration, not nmtui UI surface. |
| Device statistics via NM Statistics interface | Separate `Device.Statistics` D-Bus interface. |
| LLDP neighbors | No nmtui equivalent. |
| P2P Wi-Fi | Not in nmtui. |
| Specialized devices (OVS, SR-IOV, etc.) | Exotic connection types; add to `DeviceType` when needed. |

---

## 10. Integration into Application

```rust
// In Application::new, select backend
let backend = if cfg!(feature = "mock") {
    MockBackend::new()
} else {
    // ZbusBackend::new().await? — future work
    panic!("Real backend not yet implemented. Build with --features mock.")
};

// HomeData gets the backend handle and spawns tokio tasks:
// - list_devices + list_connections on startup
// - request_scan on 's' keypress
// - Background tasks for each subscription translate events → Message
```

The existing `Message` enum in `component.rs` is extended with backend-driven messages (e.g., `DeviceUpdated(Device)`, `ScanCompleted`) as needed by the UI layer.

---

## 11. Self-Review

- **Placeholder scan**: No TBDs or TODOs in method surface — all methods named and typed. `EapSettings` is intentionally minimal with a "deferred" note. `DeviceStateReason` / `ConnectionStateReason` are stub enums with future expansion notes.
- **Internal consistency**: Subscription methods match D-Bus interface groupings. Types are used consistently across all trait methods. `ConnectionSettings` is the input for `add_connection`, `update_connection`, and `add_and_activate_connection`.
- **Scope check**: This is a single focused spec — one trait, one mock impl, type definitions. Real `zbus` backend is deferred to a future plan.
- **Ambiguity check**: `ConnectionProfile` vs `ConnectionSettings` is clear (summary vs full editable config). `active_id` is a `String` because it maps to D-Bus object paths (opaque string identifiers), not UUIDs.

---

*Spec written 2026-05-21. Next step: invoke `writing-plans` skill to produce implementation plan.*
