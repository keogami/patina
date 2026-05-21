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