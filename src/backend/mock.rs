use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

use crate::backend::types::*;

#[derive(Clone)]
pub struct MockBackend(Arc<Mutex<MockState>>);

/// Policy for how `add_and_activate_connection` (new connections with password)
/// should behave.  Existing saved-profile activation always succeeds.
#[derive(Clone, Default)]
pub enum AuthConfig {
    #[default]
    AlwaysAccept,
    AlwaysReject {
        message: String,
    },
    Random {
        pass_rate: f64,
    },
}

// Manual Default impl replaced with derive above.

/// Iterator-driven ring of AP scan snapshots.
struct ApSnapshotRing {
    sets: Vec<Vec<AccessPoint>>,
    index: usize,
}

impl ApSnapshotRing {
    fn current(&self) -> &[AccessPoint] {
        &self.sets[self.index]
    }

    /// Advance to the next snapshot, apply ±5 signal wobble, and return the result.
    fn advance(&mut self) -> Vec<AccessPoint> {
        self.index = (self.index + 1) % self.sets.len();
        let mut aps = self.sets[self.index].clone();
        for ap in &mut aps {
            let delta = (rand::random::<i8>() % 10) as i16;
            ap.strength = (ap.strength as i16 + delta).clamp(0, 100) as u8;
        }
        aps
    }
}

struct MockState {
    devices: Vec<Device>,
    ap_ring: ApSnapshotRing,
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
    auth_config: AuthConfig,
}

/// Build a rotated AP with signal wobble.
#[allow(clippy::too_many_arguments)]
fn ap(
    ssid: &[u8],
    bssid: &str,
    strength: u8,
    freq: u32,
    ch: u32,
    bw: u32,
    bitrate: u32,
    sec: SecurityType,
    last: u32,
) -> AccessPoint {
    AccessPoint {
        ssid: ssid.to_vec(),
        bssid: bssid.into(),
        strength,
        frequency: freq,
        channel: ch,
        bandwidth: bw,
        max_bitrate: bitrate,
        security: sec,
        wps: false,
        mode: WifiMode::Infra,
        last_seen: last,
    }
}

impl MockBackend {
    pub fn new() -> Self {
        Self::with_auth_config(AuthConfig::default())
    }

    pub fn with_auth_config(auth_config: AuthConfig) -> Self {
        let wpa2 = |v| SecurityType::WpaPsk { version: v };
        let wpa2e = |v| SecurityType::WpaEnterprise { version: v };

        let s0 = vec![
            ap(
                b"Overcast-5G",
                "A4:2B:B0:FE:31:9C",
                82,
                5220,
                44,
                80,
                650_000,
                wpa2(WpaVersion::Wpa2),
                0,
            ),
            ap(
                b"Acme-Corp",
                "88:3D:24:17:02:A1",
                71,
                5750,
                149,
                80,
                867_000,
                wpa2e(WpaVersion::Wpa2),
                5,
            ),
            ap(
                b"FiberLink_9A82",
                "1C:83:41:9A:82:00",
                58,
                6130,
                37,
                160,
                1_201_000,
                wpa2(WpaVersion::Wpa3),
                12,
            ),
            ap(
                b"xfinitywifi",
                "0C:54:A5:88:2B:77",
                42,
                2412,
                1,
                20,
                150_000,
                SecurityType::Open,
                30,
            ),
            ap(
                b"TP-Link_5544",
                "E4:95:6E:55:44:A0",
                28,
                2437,
                6,
                20,
                300_000,
                wpa2(WpaVersion::Wpa2),
                60,
            ),
        ];
        let s1 = vec![
            ap(
                b"Overcast-5G",
                "A4:2B:B0:FE:31:9C",
                78,
                5220,
                44,
                80,
                650_000,
                wpa2(WpaVersion::Wpa2),
                2,
            ),
            ap(
                b"Acme-Corp",
                "88:3D:24:17:02:A1",
                74,
                5750,
                149,
                80,
                867_000,
                wpa2e(WpaVersion::Wpa2),
                7,
            ),
            ap(
                b"FiberLink_9A82",
                "1C:83:41:9A:82:00",
                62,
                6130,
                37,
                160,
                1_201_000,
                wpa2(WpaVersion::Wpa3),
                14,
            ),
            ap(
                b"xfinitywifi",
                "0C:54:A5:88:2B:77",
                45,
                2412,
                1,
                20,
                150_000,
                SecurityType::Open,
                32,
            ),
            // TP-Link gone
        ];
        let s2 = vec![
            ap(
                b"Overcast-5G",
                "A4:2B:B0:FE:31:9C",
                85,
                5220,
                44,
                80,
                650_000,
                wpa2(WpaVersion::Wpa2),
                1,
            ),
            ap(
                b"Acme-Corp",
                "88:3D:24:17:02:A1",
                68,
                5750,
                149,
                80,
                867_000,
                wpa2e(WpaVersion::Wpa2),
                6,
            ),
            ap(
                b"FiberLink_9A82",
                "1C:83:41:9A:82:00",
                55,
                6130,
                37,
                160,
                1_201_000,
                wpa2(WpaVersion::Wpa3),
                13,
            ),
            // xfinitywifi gone
            ap(
                b"TP-Link_5544",
                "E4:95:6E:55:44:A0",
                31,
                2437,
                6,
                20,
                300_000,
                wpa2(WpaVersion::Wpa2),
                61,
            ),
        ];
        let s3 = vec![
            ap(
                b"Overcast-5G",
                "A4:2B:B0:FE:31:9C",
                80,
                5220,
                44,
                80,
                650_000,
                wpa2(WpaVersion::Wpa2),
                3,
            ),
            ap(
                b"Acme-Corp",
                "88:3D:24:17:02:A1",
                70,
                5750,
                149,
                80,
                867_000,
                wpa2e(WpaVersion::Wpa2),
                8,
            ),
            ap(
                b"FiberLink_9A82",
                "1C:83:41:9A:82:00",
                60,
                6130,
                37,
                160,
                1_201_000,
                wpa2(WpaVersion::Wpa3),
                15,
            ),
            ap(
                b"xfinitywifi",
                "0C:54:A5:88:2B:77",
                40,
                2412,
                1,
                20,
                150_000,
                SecurityType::Open,
                33,
            ),
            ap(
                b"TP-Link_5544",
                "E4:95:6E:55:44:A0",
                25,
                2437,
                6,
                20,
                300_000,
                wpa2(WpaVersion::Wpa2),
                62,
            ),
        ];

        let ap_ring = ApSnapshotRing {
            sets: vec![s0, s1, s2, s3],
            index: 0,
        };

        let wlan0_uuid = Uuid::new_v4();
        let eth0_uuid = Uuid::new_v4();
        let saved_wifi_uuid = Uuid::new_v4();

        let saved_connections = vec![
            ConnectionProfile {
                uuid: wlan0_uuid,
                name: "Overcast-5G".into(),
                connection_type: ConnectionType::Wifi,
                autoconnect: true,
                interface_name: Some("wlan0".into()),
                last_used: Some(SystemTime::now() - Duration::from_secs(300)),
                unsaved: false,
                filename: Some(
                    "/etc/NetworkManager/system-connections/Overcast-5G.nmconnection".into(),
                ),
            },
            ConnectionProfile {
                uuid: eth0_uuid,
                name: "Wired".into(),
                connection_type: ConnectionType::Ethernet,
                autoconnect: true,
                interface_name: Some("eth0".into()),
                last_used: Some(SystemTime::now() - Duration::from_secs(86400)),
                unsaved: false,
                filename: Some("/etc/NetworkManager/system-connections/Wired.nmconnection".into()),
            },
            ConnectionProfile {
                uuid: saved_wifi_uuid,
                name: "CoffeeShop".into(),
                connection_type: ConnectionType::Wifi,
                autoconnect: false,
                interface_name: None,
                last_used: Some(SystemTime::now() - Duration::from_secs(604800)),
                unsaved: false,
                filename: Some(
                    "/etc/NetworkManager/system-connections/CoffeeShop.nmconnection".into(),
                ),
            },
        ];

        let active_connections = vec![
            ActiveConnection {
                id: "active-1".into(),
                profile_uuid: wlan0_uuid,
                name: "Overcast-5G".into(),
                connection_type: ConnectionType::Wifi,
                interface: "wlan0".into(),
                state: ConnectionState::Activated,
                state_flags: 0,
                default: true,
                default6: true,
                vpn: false,
                ip4_config: Some(IpConfig {
                    method: IpMethod::Auto,
                    addresses: vec![IpAddressWithPrefix {
                        address: "10.0.0.147".parse().unwrap(),
                        prefix: 24,
                    }],
                    gateway: Some("10.0.0.1".into()),
                    dns: vec!["10.0.0.1".into()],
                    search_domains: vec![],
                    routes: vec![],
                }),
                ip6_config: Some(IpConfig {
                    method: IpMethod::Auto,
                    addresses: vec![IpAddressWithPrefix {
                        address: "fe80::1".parse().unwrap(),
                        prefix: 64,
                    }],
                    gateway: None,
                    dns: vec![],
                    search_domains: vec![],
                    routes: vec![],
                }),
                devices: vec!["wlan0".into()],
            },
            ActiveConnection {
                id: "active-2".into(),
                profile_uuid: eth0_uuid,
                name: "Wired".into(),
                connection_type: ConnectionType::Ethernet,
                interface: "eth0".into(),
                state: ConnectionState::Activated,
                state_flags: 0,
                default: false,
                default6: false,
                vpn: false,
                ip4_config: Some(IpConfig {
                    method: IpMethod::Auto,
                    addresses: vec![IpAddressWithPrefix {
                        address: "192.168.1.42".parse().unwrap(),
                        prefix: 24,
                    }],
                    gateway: Some("192.168.1.1".into()),
                    dns: vec!["192.168.1.1".into()],
                    search_domains: vec![],
                    routes: vec![],
                }),
                ip6_config: None,
                devices: vec!["eth0".into()],
            },
        ];

        let mut connection_settings = HashMap::new();
        connection_settings.insert(
            wlan0_uuid,
            ConnectionSettings {
                common: CommonSettings {
                    name: "Overcast-5G".into(),
                    uuid: wlan0_uuid,
                    autoconnect: true,
                    interface_name: Some("wlan0".into()),
                    cloned_mac: None,
                    mtu: None,
                    metered: None,
                    zone: None,
                },
                connection_type: ConnectionTypeSettings::Wifi {
                    ssid: b"Overcast-5G".to_vec(),
                    mode: WifiMode::Infra,
                    bssid: None,
                    band: None,
                    channel: None,
                    security: WifiSecuritySettings::WpaPsk {
                        version: WpaVersion::Wpa2,
                        password: String::new(),
                    },
                    hidden: false,
                    powersave: None,
                },
                ipv4: IpSettings {
                    method: IpMethod::Auto,
                    addresses: vec![],
                    gateway: None,
                    dns: vec![],
                    search_domains: vec![],
                    routes: vec![],
                    never_default: false,
                    ignore_auto_dns: false,
                },
                ipv6: IpSettings {
                    method: IpMethod::Auto,
                    addresses: vec![],
                    gateway: None,
                    dns: vec![],
                    search_domains: vec![],
                    routes: vec![],
                    never_default: false,
                    ignore_auto_dns: false,
                },
            },
        );

        let mut device_stats = HashMap::new();
        device_stats.insert(
            "wlan0".into(),
            DeviceStats {
                device: "wlan0".into(),
                rx_bytes: 1_000_000,
                tx_bytes: 500_000,
                rx_packets: 10_000,
                tx_packets: 5_000,
                signal_dbm: -45,
                tx_rate: 650_000,
                rx_rate: 650_000,
            },
        );
        device_stats.insert(
            "eth0".into(),
            DeviceStats {
                device: "eth0".into(),
                rx_bytes: 10_000_000,
                tx_bytes: 5_000_000,
                rx_packets: 100_000,
                tx_packets: 50_000,
                signal_dbm: 0,
                tx_rate: 1_000_000,
                rx_rate: 1_000_000,
            },
        );

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
                Device {
                    interface: "wlan0".into(),
                    ip_interface: Some("wlan0".into()),
                    device_type: DeviceType::Wifi,
                    state: DeviceState::Activated,
                    state_reason: None,
                    managed: true,
                    autoconnect: true,
                    mac_address: "A4:2B:B0:FE:31:9C".into(),
                    mtu: 1500,
                    driver: "iwlwifi".into(),
                    driver_version: "0.1".into(),
                },
                Device {
                    interface: "eth0".into(),
                    ip_interface: Some("eth0".into()),
                    device_type: DeviceType::Ethernet,
                    state: DeviceState::Activated,
                    state_reason: None,
                    managed: true,
                    autoconnect: true,
                    mac_address: "00:1A:2B:3C:4D:5E".into(),
                    mtu: 1500,
                    driver: "e1000e".into(),
                    driver_version: "0.1".into(),
                },
            ],
            ap_ring,
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
            auth_config,
        };

        Self(Arc::new(Mutex::new(state)))
    }
}

impl crate::backend::NetworkManagerBackend for MockBackend {
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
            Ok(state
                .devices
                .iter()
                .find(|d| d.interface == interface)
                .cloned())
        }
    }

    fn get_access_points(
        &self,
        _interface: &str,
    ) -> impl Future<Output = Result<Vec<AccessPoint>>> + Send {
        let backend = self.clone();
        async move {
            let state = backend.0.lock().await;
            Ok(state.ap_ring.current().to_vec())
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
                // Advance the ring to the next snapshot (with wobble baked in).
                let _ = state.ap_ring.advance();

                // Emit ScanDone to AP subscribers for this interface.
                if let Some(txs) = state.ap_event_txs.get(&interface) {
                    let event = ApEvent::ScanDone {
                        interface: interface.clone(),
                    };
                    for tx in txs.iter() {
                        let _ = tx.send(event.clone());
                    }
                }
            }
            Ok(())
        }
    }

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

    fn get_connection_by_uuid(
        &self,
        uuid: &Uuid,
    ) -> impl Future<Output = Result<ConnectionProfile>> + Send {
        let backend = self.clone();
        let uuid = *uuid;
        async move {
            let state = backend.0.lock().await;
            state
                .saved_connections
                .iter()
                .find(|c| c.uuid == uuid)
                .cloned()
                .context("Connection not found")
        }
    }
    fn activate_connection(
        &self,
        uuid: &Uuid,
        interface: Option<&str>,
    ) -> impl Future<Output = Result<ActiveConnection>> + Send {
        let backend = self.clone();
        let uuid = *uuid;
        let interface = interface.map(|s| s.to_string());
        async move {
            let mut state = backend.0.lock().await;
            let profile = state
                .saved_connections
                .iter()
                .find(|c| c.uuid == uuid)
                .cloned()
                .context("Connection not found")?;

            let id = format!("active-{}", state.next_active_id);
            state.next_active_id += 1;

            let interface = interface.unwrap_or_else(|| {
                profile
                    .interface_name
                    .clone()
                    .unwrap_or_else(|| "unknown".into())
            });

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
                    addresses: vec![IpAddressWithPrefix {
                        address: "192.168.1.100".parse().unwrap(),
                        prefix: 24,
                    }],
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
            let idx = state
                .active_connections
                .iter()
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
            let to_remove: Vec<String> = state
                .active_connections
                .iter()
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
    fn add_and_activate_connection(
        &self,
        settings: &ConnectionSettings,
        interface: Option<&str>,
    ) -> impl Future<Output = Result<ActiveConnection>> + Send {
        let backend = self.clone();
        let settings = settings.clone();
        let interface = interface.map(|s| s.to_string());
        async move {
            // Apply auth policy for new connections only.
            {
                let state = backend.0.lock().await;
                match &state.auth_config {
                    AuthConfig::AlwaysAccept => {}
                    AuthConfig::AlwaysReject { message } => {
                        anyhow::bail!("{}", message);
                    }
                    AuthConfig::Random { pass_rate } => {
                        if rand::random::<f64>() > *pass_rate || *pass_rate <= 0.0 {
                            anyhow::bail!("wrong password for mock network");
                        }
                    }
                }
            }
            let profile = backend.add_connection(&settings).await?;
            backend
                .activate_connection(&profile.uuid, interface.as_deref())
                .await
        }
    }
    fn get_secrets(
        &self,
        _uuid: &Uuid,
        _setting_name: &str,
    ) -> impl Future<Output = Result<Secrets>> + Send {
        async move { Ok(Secrets::new()) }
    }
    fn get_settings(&self, uuid: &Uuid) -> impl Future<Output = Result<ConnectionSettings>> + Send {
        let backend = self.clone();
        let uuid = *uuid;
        async move {
            let state = backend.0.lock().await;
            state
                .connection_settings
                .get(&uuid)
                .cloned()
                .context("Settings not found")
        }
    }
    fn update_connection(
        &self,
        uuid: &Uuid,
        settings: &ConnectionSettings,
    ) -> impl Future<Output = Result<()>> + Send {
        let backend = self.clone();
        let uuid = *uuid;
        let settings = settings.clone();
        async move {
            let mut state = backend.0.lock().await;
            if !state.connection_settings.contains_key(&uuid) {
                anyhow::bail!("Connection settings not found");
            }
            let name = settings.common.name.clone();
            state.connection_settings.insert(uuid, settings);

            // Update profile name if changed
            if let Some(conn) = state.saved_connections.iter_mut().find(|c| c.uuid == uuid) {
                conn.name = name;
                conn.unsaved = true;
            }

            for tx in state.profile_event_txs.iter() {
                let _ = tx.send(ProfileEvent::Updated { uuid });
            }
            Ok(())
        }
    }
    fn add_connection(
        &self,
        settings: &ConnectionSettings,
    ) -> impl Future<Output = Result<ConnectionProfile>> + Send {
        let backend = self.clone();
        let settings = settings.clone();
        async move {
            let mut state = backend.0.lock().await;
            let profile = ConnectionProfile {
                uuid: settings.common.uuid,
                name: settings.common.name.clone(),
                connection_type: match settings.connection_type {
                    ConnectionTypeSettings::Wifi { .. } => ConnectionType::Wifi,
                    ConnectionTypeSettings::Ethernet { .. } => ConnectionType::Ethernet,
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
                filename: Some(format!(
                    "/etc/NetworkManager/system-connections/{}.nmconnection",
                    settings.common.name
                )),
            };

            state.saved_connections.push(profile.clone());
            state.connection_settings.insert(profile.uuid, settings);

            // Emit event to subscribers
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
            let idx = state
                .saved_connections
                .iter()
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
    fn get_global_state(&self) -> impl Future<Output = Result<GlobalState>> + Send {
        let backend = self.clone();
        async move {
            let state = backend.0.lock().await;
            Ok(state.global_state.clone())
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
    fn save_hostname(&self, hostname: &str) -> impl Future<Output = Result<()>> + Send {
        let backend = self.clone();
        let hostname = hostname.to_string();
        async move {
            let mut state = backend.0.lock().await;
            state.global_state.hostname = hostname;
            Ok(())
        }
    }
    fn set_device_managed(
        &self,
        interface: &str,
        managed: bool,
    ) -> impl Future<Output = Result<()>> + Send {
        let backend = self.clone();
        let interface = interface.to_string();
        async move {
            let mut state = backend.0.lock().await;
            if let Some(dev) = state.devices.iter_mut().find(|d| d.interface == interface) {
                dev.managed = managed;
                for tx in state.device_event_txs.iter() {
                    let _ = tx.send(DeviceEvent::ManagedChanged {
                        interface: interface.clone(),
                        managed,
                    });
                }
            }
            Ok(())
        }
    }
    fn set_device_autoconnect(
        &self,
        interface: &str,
        autoconnect: bool,
    ) -> impl Future<Output = Result<()>> + Send {
        let backend = self.clone();
        let interface = interface.to_string();
        async move {
            let mut state = backend.0.lock().await;
            if let Some(dev) = state.devices.iter_mut().find(|d| d.interface == interface) {
                dev.autoconnect = autoconnect;
                for tx in state.device_event_txs.iter() {
                    let _ = tx.send(DeviceEvent::AutoconnectChanged {
                        interface: interface.clone(),
                        autoconnect,
                    });
                }
            }
            Ok(())
        }
    }
    fn get_device_stats(
        &self,
        interface: &str,
    ) -> impl Future<Output = Result<DeviceStats>> + Send {
        let backend = self.clone();
        let interface = interface.to_string();
        async move {
            let mut state = backend.0.lock().await;
            let stats = state
                .device_stats
                .entry(interface.clone())
                .or_insert_with(|| DeviceStats {
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
    fn subscribe_device_events(
        &self,
    ) -> impl Future<Output = Result<mpsc::UnboundedReceiver<DeviceEvent>>> + Send {
        let backend = self.clone();
        async move {
            let (tx, rx) = mpsc::unbounded_channel();
            let mut state = backend.0.lock().await;
            state.device_event_txs.push(tx);
            Ok(rx)
        }
    }
    fn subscribe_ap_events(
        &self,
        interface: &str,
    ) -> impl Future<Output = Result<mpsc::UnboundedReceiver<ApEvent>>> + Send {
        let backend = self.clone();
        let interface = interface.to_string();
        async move {
            let (tx, rx) = mpsc::unbounded_channel();
            let mut state = backend.0.lock().await;
            state.ap_event_txs.entry(interface).or_default().push(tx);
            Ok(rx)
        }
    }
    fn subscribe_profile_events(
        &self,
    ) -> impl Future<Output = Result<mpsc::UnboundedReceiver<ProfileEvent>>> + Send {
        let backend = self.clone();
        async move {
            let (tx, rx) = mpsc::unbounded_channel();
            let mut state = backend.0.lock().await;
            state.profile_event_txs.push(tx);
            Ok(rx)
        }
    }
    fn subscribe_active_events(
        &self,
    ) -> impl Future<Output = Result<mpsc::UnboundedReceiver<ActiveEvent>>> + Send {
        let backend = self.clone();
        async move {
            let (tx, rx) = mpsc::unbounded_channel();
            let mut state = backend.0.lock().await;
            state.active_event_txs.push(tx);
            Ok(rx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NetworkManagerBackend;

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

    #[tokio::test]
    async fn mock_activates_connection() {
        let backend = MockBackend::new();
        let conns = backend.list_connections().await.unwrap();
        let target = conns.iter().find(|c| c.name == "CoffeeShop").unwrap();

        let active = backend
            .activate_connection(&target.uuid, Some("wlan0"))
            .await
            .unwrap();
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
                security: WifiSecuritySettings::WpaPsk {
                    version: WpaVersion::Wpa2,
                    password: "secret".into(),
                },
                hidden: false,
                powersave: None,
            },
            ipv4: IpSettings {
                method: IpMethod::Auto,
                addresses: vec![],
                gateway: None,
                dns: vec![],
                search_domains: vec![],
                routes: vec![],
                never_default: false,
                ignore_auto_dns: false,
            },
            ipv6: IpSettings {
                method: IpMethod::Auto,
                addresses: vec![],
                gateway: None,
                dns: vec![],
                search_domains: vec![],
                routes: vec![],
                never_default: false,
                ignore_auto_dns: false,
            },
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
        backend
            .update_connection(&profile.uuid, &updated)
            .await
            .unwrap();

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

    #[tokio::test]
    async fn mock_subscription_receives_scan_done() {
        let backend = MockBackend::new();
        let mut rx = backend.subscribe_ap_events("wlan0").await.unwrap();

        backend.request_scan("wlan0").await.unwrap();

        // Wait up to 1 second for the event
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await;
        assert!(event.is_ok());
        assert!(matches!(event.unwrap(), Some(ApEvent::ScanDone { .. })));
    }

    #[tokio::test]
    async fn mock_subscription_receives_device_events() {
        let backend = MockBackend::new();
        let mut rx = backend.subscribe_device_events().await.unwrap();

        backend.set_device_managed("wlan0", false).await.unwrap();

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await;
        assert!(event.is_ok());
        assert!(
            matches!(event.unwrap(), Some(DeviceEvent::ManagedChanged { interface, managed }) if interface == "wlan0" && !managed)
        );
    }

    #[tokio::test]
    async fn mock_ap_ring_advances_on_scan() {
        let backend = MockBackend::new();
        let aps0 = backend.get_access_points("wlan0").await.unwrap();

        // Scan advances ring: s0 (5) → s1 (4)
        backend.request_scan("wlan0").await.unwrap();
        let aps1 = backend.get_access_points("wlan0").await.unwrap();
        assert_ne!(aps0.len(), aps1.len(), "AP count should change after scan");

        // Three more scans wrap back to s0 (5)
        backend.request_scan("wlan0").await.unwrap();
        backend.request_scan("wlan0").await.unwrap();
        backend.request_scan("wlan0").await.unwrap();
        let aps4 = backend.get_access_points("wlan0").await.unwrap();
        assert_eq!(
            aps0.len(),
            aps4.len(),
            "AP count should cycle back after 4 scans"
        );
    }

    #[tokio::test]
    async fn mock_auth_always_reject() {
        let backend = MockBackend::with_auth_config(AuthConfig::AlwaysReject {
            message: "bad password".into(),
        });
        let settings = ConnectionSettings {
            common: CommonSettings {
                name: "FailNet".into(),
                uuid: Uuid::new_v4(),
                autoconnect: true,
                interface_name: None,
                cloned_mac: None,
                mtu: None,
                metered: None,
                zone: None,
            },
            connection_type: ConnectionTypeSettings::Wifi {
                ssid: b"FailNet".to_vec(),
                mode: WifiMode::Infra,
                bssid: None,
                band: None,
                channel: None,
                security: WifiSecuritySettings::WpaPsk {
                    version: WpaVersion::Wpa2,
                    password: "wrong".into(),
                },
                hidden: false,
                powersave: None,
            },
            ipv4: IpSettings {
                method: IpMethod::Auto,
                addresses: vec![],
                gateway: None,
                dns: vec![],
                search_domains: vec![],
                routes: vec![],
                never_default: false,
                ignore_auto_dns: false,
            },
            ipv6: IpSettings {
                method: IpMethod::Auto,
                addresses: vec![],
                gateway: None,
                dns: vec![],
                search_domains: vec![],
                routes: vec![],
                never_default: false,
                ignore_auto_dns: false,
            },
        };
        let err = backend
            .add_and_activate_connection(&settings, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("bad password"));
    }

    #[tokio::test]
    async fn mock_auth_random_eventually_accepts() {
        // With pass_rate=1.0 it should always succeed.
        let backend = MockBackend::with_auth_config(AuthConfig::Random { pass_rate: 1.0 });
        let settings = ConnectionSettings {
            common: CommonSettings {
                name: "SureNet".into(),
                uuid: Uuid::new_v4(),
                autoconnect: true,
                interface_name: None,
                cloned_mac: None,
                mtu: None,
                metered: None,
                zone: None,
            },
            connection_type: ConnectionTypeSettings::Wifi {
                ssid: b"SureNet".to_vec(),
                mode: WifiMode::Infra,
                bssid: None,
                band: None,
                channel: None,
                security: WifiSecuritySettings::WpaPsk {
                    version: WpaVersion::Wpa2,
                    password: "secret".into(),
                },
                hidden: false,
                powersave: None,
            },
            ipv4: IpSettings {
                method: IpMethod::Auto,
                addresses: vec![],
                gateway: None,
                dns: vec![],
                search_domains: vec![],
                routes: vec![],
                never_default: false,
                ignore_auto_dns: false,
            },
            ipv6: IpSettings {
                method: IpMethod::Auto,
                addresses: vec![],
                gateway: None,
                dns: vec![],
                search_domains: vec![],
                routes: vec![],
                never_default: false,
                ignore_auto_dns: false,
            },
        };
        let res = backend.add_and_activate_connection(&settings, None).await;
        assert!(res.is_ok(), "pass_rate=1.0 should always succeed");
    }

    #[tokio::test]
    async fn mock_auth_random_eventually_rejects() {
        // With pass_rate=0.0 it should always fail.
        let backend = MockBackend::with_auth_config(AuthConfig::Random { pass_rate: 0.0 });
        let settings = ConnectionSettings {
            common: CommonSettings {
                name: "NeverNet".into(),
                uuid: Uuid::new_v4(),
                autoconnect: true,
                interface_name: None,
                cloned_mac: None,
                mtu: None,
                metered: None,
                zone: None,
            },
            connection_type: ConnectionTypeSettings::Wifi {
                ssid: b"NeverNet".to_vec(),
                mode: WifiMode::Infra,
                bssid: None,
                band: None,
                channel: None,
                security: WifiSecuritySettings::WpaPsk {
                    version: WpaVersion::Wpa2,
                    password: "secret".into(),
                },
                hidden: false,
                powersave: None,
            },
            ipv4: IpSettings {
                method: IpMethod::Auto,
                addresses: vec![],
                gateway: None,
                dns: vec![],
                search_domains: vec![],
                routes: vec![],
                never_default: false,
                ignore_auto_dns: false,
            },
            ipv6: IpSettings {
                method: IpMethod::Auto,
                addresses: vec![],
                gateway: None,
                dns: vec![],
                search_domains: vec![],
                routes: vec![],
                never_default: false,
                ignore_auto_dns: false,
            },
        };
        let err = backend
            .add_and_activate_connection(&settings, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("wrong password"));
    }
}
