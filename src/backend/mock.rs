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
    fn get_secrets(&self, _uuid: &Uuid, _setting_name: &str) -> impl Future<Output = Result<Secrets>> + Send {
        async move { Ok(Secrets::new()) }
    }
    fn get_settings(&self, _uuid: &Uuid) -> impl Future<Output = Result<ConnectionSettings>> + Send {
        async move { anyhow::bail!("not yet implemented") }
    }
    fn update_connection(&self, _uuid: &Uuid, _settings: &ConnectionSettings) -> impl Future<Output = Result<()>> + Send {
        async move { anyhow::bail!("not yet implemented") }
    }
    fn add_connection(&self, settings: &ConnectionSettings) -> impl Future<Output = Result<ConnectionProfile>> + Send {
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
                filename: Some(format!("/etc/NetworkManager/system-connections/{}.nmconnection", settings.common.name)),
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
}
