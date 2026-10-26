# NetworkManager Backend Integration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the async `NetworkManagerBackend` trait into the synchronous message-passing UI event loop using a `BackendBridge` actor, replacing inline mock data with real backend calls.

**Architecture:** A `BackendBridge` tokio coordinator spawns detached child tasks per `BridgeCommand`. Child tasks call backend methods and send results back as `Message::BridgeResult` through the existing `std::sync::mpsc` channel. `Application` owns the bridge, intercepts user-intent messages, and tracks startup loading. `HomeData` handles `BridgeResult` variants and maps backend types to UI types inline.

**Tech Stack:** Rust 2024, tokio, ratatui 0.30, anyhow, enum_dispatch, circular-buffer

---

## File Map

| File | Responsibility |
|------|-------------|
| `src/backend/bridge.rs` | `BridgeCommand`, `BridgeResult`, `SubscriptionHandle`, `BackendBridge` coordinator |
| `src/backend/mod.rs` | Re-export bridge module alongside existing types/trait |
| `src/application/component.rs` | Extend `Message` with bridge results and user-intent variants |
| `src/application/mod.rs` | `Application` owns bridge, intercepts messages, tracks startup loading |
| `src/application/home.rs` | Remove inline mock loading; add `BridgeResult` handlers; inline backend→UI mappers |
| `src/application/home/authenticate.rs` | Return `AuthSubmitted` on Enter instead of `FinishAuthentication` |
| `src/main.rs` | Create channel, create backend, create bridge, pass to `Application` |

---

## Task 1: Bridge Protocol Types

**Files:**
- Create: `src/backend/bridge.rs`

- [ ] **Step 1: Write `BridgeCommand` enum**

```rust
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;
use crate::backend::types::*;

#[derive(Debug, Clone)]
pub enum BridgeCommand {
    // Discovery
    ListDevices,
    ListAccessPoints { interface: String },
    RequestScan { interface: String },

    // Connection Control
    ListConnections,
    GetActiveConnections,
    ActivateConnection { uuid: Uuid, interface: Option<String> },
    DeactivateConnection { active_id: String },
    DisconnectDevice { interface: String },
    AddAndActivateConnection { settings: ConnectionSettings, interface: Option<String> },

    // Configuration
    GetSettings { uuid: Uuid },
    UpdateConnection { uuid: Uuid, settings: ConnectionSettings },
    AddConnection { settings: ConnectionSettings },
    DeleteConnection { uuid: Uuid },
    GetSecrets { uuid: Uuid, setting_name: String },

    // Global State
    GetGlobalState,
    SetNetworkingEnabled(bool),
    SetWirelessEnabled(bool),
    SetWwanEnabled(bool),
    SaveHostname(String),
    SetDeviceManaged { interface: String, managed: bool },
    SetDeviceAutoconnect { interface: String, autoconnect: bool },

    // Stats
    GetDeviceStats { interface: String },

    // Subscriptions
    SubscribeDeviceEvents,
    SubscribeApEvents { interface: String },
    SubscribeProfileEvents,
    SubscribeActiveEvents,
    Unsubscribe { handle: SubscriptionHandle },
}
```

- [ ] **Step 2: Write `BridgeResult` enum**

```rust
#[derive(Debug, Clone)]
pub enum BridgeResult {
    // One-shot results
    DevicesLoaded(anyhow::Result<Vec<Device>>),
    AccessPointsLoaded(anyhow::Result<Vec<AccessPoint>>),
    ConnectionsLoaded(anyhow::Result<Vec<ConnectionProfile>>),
    ActiveConnectionsLoaded(anyhow::Result<Vec<ActiveConnection>>),
    GlobalStateLoaded(anyhow::Result<GlobalState>),
    ScanCompleted { interface: String, result: anyhow::Result<()> },
    ConnectionActivated { uuid: Uuid, result: anyhow::Result<ActiveConnection> },
    ConnectionDeactivated { active_id: String, result: anyhow::Result<()> },
    DeviceDisconnected { interface: String, result: anyhow::Result<()> },
    SettingsLoaded { uuid: Uuid, result: anyhow::Result<ConnectionSettings> },
    ConnectionUpdated { uuid: Uuid, result: anyhow::Result<()> },
    ConnectionAdded { result: anyhow::Result<ConnectionProfile> },
    ConnectionDeleted { uuid: Uuid, result: anyhow::Result<()> },
    SecretsLoaded { uuid: Uuid, result: anyhow::Result<Secrets> },
    DeviceStatsLoaded { interface: String, result: anyhow::Result<DeviceStats> },
    SubscriptionStarted { handle: SubscriptionHandle },

    // Push events
    DeviceEvent(DeviceEvent),
    ApEvent(ApEvent),
    ProfileEvent(ProfileEvent),
    ActiveEvent(ActiveEvent),
}
```

- [ ] **Step 3: Write `SubscriptionHandle`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionHandle(pub u64);
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --features mock`
Expected: PASS (only types, no impl yet)

- [ ] **Step 5: Commit**

```bash
git add src/backend/bridge.rs
git commit -m "feat: add bridge protocol types"
```

---

## Task 2: Wire Bridge into Backend Module

**Files:**
- Modify: `src/backend/mod.rs`

- [ ] **Step 1: Add bridge re-export**

Insert after `pub mod types;`:
```rust
pub mod bridge;
pub use bridge::*;
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features mock`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/backend/mod.rs
git commit -m "feat: re-export bridge module from backend"
```

---

## Task 3: Extend Message Enum

**Files:**
- Modify: `src/application/component.rs`

- [ ] **Step 1: Import bridge result and add new variants**

Add import:
```rust
use crate::backend::bridge::BridgeResult;
```

Extend `Message`:
```rust
pub enum Message {
    // Existing variants preserved
    Crossterm(crossterm::event::Event),
    GlobalTick,
    LoadConnected(Connected),
    LoadAvailable(Available),
    FinishLoading,
    Authenticate(AuthenticationData),
    FinishAuthentication,
    OpenInfo(InfoTarget),
    FinishInfo,

    // Backend bridge results
    BridgeResult(BridgeResult),

    // User-intent messages (intercepted by Application)
    RequestConnect { uuid: Uuid, interface: Option<String> },
    RequestDisconnect { active_id: String },
    RequestScan { interface: String },
    RequestDeviceList,
    RequestConnectionList,
    AuthSubmitted { password: String, autoconnect: bool },
    RequestAddAndActivate { settings: ConnectionSettings, interface: Option<String> },
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features mock`
Expected: PASS (new variants unused yet)

- [ ] **Step 3: Commit**

```bash
git add src/application/component.rs
git commit -m "feat: extend Message enum with bridge results and user intents"
```

---

## Task 4: Implement BackendBridge Coordinator

**Files:**
- Modify: `src/backend/bridge.rs`

- [ ] **Step 1: Add imports for coordinator**

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::application::component::Message;
use crate::backend::types::*;
use crate::backend::NetworkManagerBackend;
```

- [ ] **Step 2: Write `BackendBridge` struct and constructor**

```rust
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
                    BridgeCommand::ListDevices => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_all_devices().await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::DevicesLoaded(res)));
                        });
                    }
                    BridgeCommand::ListAccessPoints { interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_access_points(&interface).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::AccessPointsLoaded(res)));
                        });
                    }
                    BridgeCommand::RequestScan { interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.request_scan(&interface).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ScanCompleted {
                                interface,
                                result: res,
                            }));
                        });
                    }

                    // ── One-shot connection control ──
                    BridgeCommand::ListConnections => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.list_connections().await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionsLoaded(res)));
                        });
                    }
                    BridgeCommand::GetActiveConnections => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_active_connections().await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ActiveConnectionsLoaded(res)));
                        });
                    }
                    BridgeCommand::ActivateConnection { uuid, interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.activate_connection(&uuid, interface.as_deref()).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionActivated {
                                uuid,
                                result: res,
                            }));
                        });
                    }
                    BridgeCommand::DeactivateConnection { active_id } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.deactivate_connection(&active_id).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionDeactivated {
                                active_id,
                                result: res,
                            }));
                        });
                    }
                    BridgeCommand::DisconnectDevice { interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.disconnect_device(&interface).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::DeviceDisconnected {
                                interface,
                                result: res,
                            }));
                        });
                    }
                    BridgeCommand::AddAndActivateConnection { settings, interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.add_and_activate_connection(&settings, interface.as_deref()).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionActivated {
                                uuid: settings.common.uuid,
                                result: res,
                            }));
                        });
                    }

                    // ── One-shot configuration ──
                    BridgeCommand::GetSettings { uuid } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_settings(&uuid).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::SettingsLoaded { uuid, result: res }));
                        });
                    }
                    BridgeCommand::UpdateConnection { uuid, settings } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.update_connection(&uuid, &settings).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionUpdated { uuid, result: res }));
                        });
                    }
                    BridgeCommand::AddConnection { settings } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.add_connection(&settings).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionAdded { result: res }));
                        });
                    }
                    BridgeCommand::DeleteConnection { uuid } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.delete_connection(&uuid).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::ConnectionDeleted { uuid, result: res }));
                        });
                    }
                    BridgeCommand::GetSecrets { uuid, setting_name } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_secrets(&uuid, &setting_name).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::SecretsLoaded { uuid, result: res }));
                        });
                    }

                    // ── Global state ──
                    BridgeCommand::GetGlobalState => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_global_state().await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::GlobalStateLoaded(res)));
                        });
                    }
                    BridgeCommand::SetNetworkingEnabled(v) => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.set_networking_enabled(v).await;
                            // fire-and-forget; no result message
                            log::info!("set_networking_enabled: {:?}", res);
                        });
                    }
                    BridgeCommand::SetWirelessEnabled(v) => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.set_wireless_enabled(v).await;
                            log::info!("set_wireless_enabled: {:?}", res);
                        });
                    }
                    BridgeCommand::SetWwanEnabled(v) => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.set_wwan_enabled(v).await;
                            log::info!("set_wwan_enabled: {:?}", res);
                        });
                    }
                    BridgeCommand::SaveHostname(hostname) => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.save_hostname(&hostname).await;
                            log::info!("save_hostname: {:?}", res);
                        });
                    }
                    BridgeCommand::SetDeviceManaged { interface, managed } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.set_device_managed(&interface, managed).await;
                            log::info!("set_device_managed: {:?}", res);
                        });
                    }
                    BridgeCommand::SetDeviceAutoconnect { interface, autoconnect } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.set_device_autoconnect(&interface, autoconnect).await;
                            log::info!("set_device_autoconnect: {:?}", res);
                        });
                    }

                    // ── Stats ──
                    BridgeCommand::GetDeviceStats { interface } => {
                        let b = backend.clone();
                        let tx = ui_tx.clone();
                        tokio::spawn(async move {
                            let res = b.get_device_stats(&interface).await;
                            let _ = tx.send(Message::BridgeResult(BridgeResult::DeviceStatsLoaded { interface, result: res }));
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
                                    let _ = tx.send(Message::BridgeResult(BridgeResult::SubscriptionStarted { handle }));
                                    loop {
                                        tokio::select! {
                                            _ = child_token.cancelled() => break,
                                            Some(ev) = rx.recv() => {
                                                let _ = tx.send(Message::BridgeResult(BridgeResult::DeviceEvent(ev)));
                                            }
                                            else => break,
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("subscribe_device_events failed: {}", e);
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
                                    let _ = tx.send(Message::BridgeResult(BridgeResult::SubscriptionStarted { handle }));
                                    loop {
                                        tokio::select! {
                                            _ = child_token.cancelled() => break,
                                            Some(ev) = rx.recv() => {
                                                let _ = tx.send(Message::BridgeResult(BridgeResult::ApEvent(ev)));
                                            }
                                            else => break,
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("subscribe_ap_events failed: {}", e);
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
                                    let _ = tx.send(Message::BridgeResult(BridgeResult::SubscriptionStarted { handle }));
                                    loop {
                                        tokio::select! {
                                            _ = child_token.cancelled() => break,
                                            Some(ev) = rx.recv() => {
                                                let _ = tx.send(Message::BridgeResult(BridgeResult::ProfileEvent(ev)));
                                            }
                                            else => break,
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("subscribe_profile_events failed: {}", e);
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
                                    let _ = tx.send(Message::BridgeResult(BridgeResult::SubscriptionStarted { handle }));
                                    loop {
                                        tokio::select! {
                                            _ = child_token.cancelled() => break,
                                            Some(ev) = rx.recv() => {
                                                let _ = tx.send(Message::BridgeResult(BridgeResult::ActiveEvent(ev)));
                                            }
                                            else => break,
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("subscribe_active_events failed: {}", e);
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
```

- [ ] **Step 3: Add `tokio-util` dependency for `CancellationToken`**

In `Cargo.toml`:
```toml
tokio-util = "0.7"
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --features mock`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/backend/bridge.rs
git commit -m "feat: implement BackendBridge coordinator with subscriptions"
```

---

## Task 5: Update Application for Bridge Ownership

**Files:**
- Modify: `src/application/mod.rs`

- [ ] **Step 1: Import bridge and add fields to `Application`**

Add imports:
```rust
use std::sync::atomic::{AtomicU32, Ordering};
use crate::backend::bridge::{BackendBridge, BridgeCommand, BridgeResult};
```

Add fields to `Application`:
```rust
pub struct Application {
    context: Arc<RichContext>,
    pane: Pane,
    message_tx: Arc<std::sync::mpsc::Sender<Message>>,
    message_rx: Option<std::sync::mpsc::Receiver<Message>>,
    bridge: Arc<BackendBridge>,
    startup_pending: AtomicU32,
}
```

- [ ] **Step 2: Update `Application::new` signature and body**

```rust
impl Application {
    pub fn new(
        ctx: Context,
        bridge: Arc<BackendBridge>,
        message_tx: Arc<std::sync::mpsc::Sender<Message>>,
        message_rx: std::sync::mpsc::Receiver<Message>,
    ) -> Self {
        let context = RichContext {
            log_file: ctx.log_file,
            fps: ctx.fps,
            message: message_tx.clone(),
            connection_maxitem: ctx.connection_maxitem,
        };

        Self {
            pane: Pane::Home(HomeData::new()),
            context: context.into(),
            message_tx,
            message_rx: Some(message_rx),
            bridge,
            startup_pending: AtomicU32::new(0),
        }
    }
```

- [ ] **Step 3: Update `run` to send startup commands**

```rust
pub fn run(mut self) -> anyhow::Result<()> {
    self.setup_event_polling()?;
    let rx = self.message_rx.take().unwrap();

    // Startup: load devices, connections, active connections
    self.bridge.send(BridgeCommand::ListDevices);
    self.bridge.send(BridgeCommand::ListConnections);
    self.bridge.send(BridgeCommand::GetActiveConnections);
    self.startup_pending.store(3, Ordering::SeqCst);

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
```

- [ ] **Step 4: Update `update` to intercept messages**

```rust
pub fn update(&mut self, ev: Message) {
    match ev {
        // ── Intercept user-intent messages → bridge commands ──
        Message::RequestConnect { uuid, interface } => {
            self.bridge.send(BridgeCommand::ActivateConnection { uuid, interface });
            return;
        }
        Message::RequestDisconnect { active_id } => {
            self.bridge.send(BridgeCommand::DeactivateConnection { active_id });
            return;
        }
        Message::RequestScan { interface } => {
            self.bridge.send(BridgeCommand::RequestScan { interface });
            return;
        }
        Message::RequestDeviceList => {
            self.bridge.send(BridgeCommand::ListDevices);
            return;
        }
        Message::RequestConnectionList => {
            self.bridge.send(BridgeCommand::ListConnections);
            return;
        }
        Message::RequestAddAndActivate { settings, interface } => {
            self.bridge.send(BridgeCommand::AddAndActivateConnection { settings, interface });
            return;
        }

        // ── Intercept bridge results → startup loading + forward to pane ──
        Message::BridgeResult(ref result) => {
            // Startup completion tracking
            let is_startup_result = matches!(
                result,
                BridgeResult::DevicesLoaded(_)
                    | BridgeResult::ConnectionsLoaded(_)
                    | BridgeResult::ActiveConnectionsLoaded(_)
                    | BridgeResult::AccessPointsLoaded(_)
            );
            if is_startup_result {
                let prev = self.startup_pending.fetch_sub(1, Ordering::SeqCst);
                if prev == 1 {
                    let _ = self.message_tx.send(Message::FinishLoading);
                }
            }

            // Forward device list to trigger AP listing for WiFi devices
            if let Message::BridgeResult(BridgeResult::DevicesLoaded(Ok(ref devices))) = ev {
                for dev in devices {
                    if matches!(dev.device_type, crate::backend::types::DeviceType::Wifi) {
                        self.bridge.send(BridgeCommand::ListAccessPoints {
                            interface: dev.interface.clone(),
                        });
                        self.startup_pending.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }

            self.pane.update(&self.context, ev);
            return;
        }

        _ => {}
    }

    self.pane.update(&self.context, ev);
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check --features mock`
Expected: PASS (HomeData::new signature changes in next task, may show error)

Note: If `HomeData::new` still takes `&RichContext`, this compiles since we're passing nothing. The next task removes the `&RichContext` param.

- [ ] **Step 6: Commit**

```bash
git add src/application/mod.rs
git commit -m "feat: Application owns bridge and intercepts user intents"
```

---

## Task 6: Update HomeData — Remove Mock Loading, Add Bridge Handlers

**Files:**
- Modify: `src/application/home.rs`
- Modify: `src/application/home/authenticate.rs`

### Part A: HomeData constructor and storage

- [ ] **Step 1: Remove `&RichContext` from `HomeData::new` and add backend data fields**

Change signature:
```rust
impl HomeData {
    pub fn new() -> Self {
```

Add fields to `HomeData`:
```rust
pub struct HomeData {
    loading: Option<Spinner>,
    connected: Connected,
    available: Available,
    selected: Selected,
    connected_scroll_state: ScrollState,
    available_scroll_state: ScrollState,
    available_max_items: Cell<usize>,
    authenticate: Option<authenticate::Authenticate>,
    info: Option<Info>,
    // NEW: backend data caches for mapping
    devices: Vec<crate::backend::types::Device>,
    profiles: Vec<crate::backend::types::ConnectionProfile>,
    active_connections: Vec<crate::backend::types::ActiveConnection>,
}
```

Update `new()`:
```rust
pub fn new() -> Self {
    Self {
        loading: Some(Spinner::new(SpinnerType::dot(), 144)),
        connected: Connected { list: Vec::new() },
        available: Available { list: Vec::new() },
        selected: Selected::None,
        connected_scroll_state: ScrollState::new(),
        available_scroll_state: ScrollState::new(),
        available_max_items: Cell::new(5),
        authenticate: None,
        info: None,
        devices: Vec::new(),
        profiles: Vec::new(),
        active_connections: Vec::new(),
    }
}
```

- [ ] **Step 2: Remove the inline mock loading block from `HomeData::new`**

Delete the entire `tokio::spawn` block that creates mock `AvailableAPDetails` and `ConnectionData` arrays and sends `LoadConnected`, `LoadAvailable`, `FinishLoading`.

### Part B: Inline mappers

- [ ] **Step 3: Add `profile_to_inactive_data` mapper**

```rust
use std::time::{Duration, SystemTime};

fn profile_to_inactive_data(profile: &crate::backend::types::ConnectionProfile) -> ConnectionData {
    let last_used = profile
        .last_used
        .and_then(|t| t.elapsed().ok())
        .unwrap_or(Duration::from_secs(0));

    ConnectionData::WifiInactive {
        name: profile.name.clone().into(),
        last_used,
        autoconnect: profile.autoconnect,
        metered: false,
        tag: "WiFi".into(),
        security: "WPA2".into(),
        bssid: None,
        saved_ip_method: "auto".into(),
        saved_dns: Vec::new(),
    }
}
```

- [ ] **Step 4: Add `active_to_connection_data` mapper**

```rust
fn active_to_connection_data(active: &crate::backend::types::ActiveConnection) -> ConnectionData {
    let ip4 = active.ip4_config.as_ref();
    let ip_str = ip4
        .and_then(|c| c.addresses.first())
        .map(|a| format!("{}/{}", a.address, a.prefix))
        .unwrap_or_else(|| "--".into());

    let gateway = ip4
        .and_then(|c| c.gateway.clone())
        .unwrap_or_else(|| "--".into());

    let dns: Vec<CowStr> = ip4
        .map(|c| c.dns.iter().map(|s| s.clone().into()).collect())
        .unwrap_or_default();

    match active.connection_type {
        crate::backend::types::ConnectionType::Wifi => ConnectionData::WifiActive {
            strength: 0.0,
            name: active.name.clone().into(),
            interface: active.interface.clone().into(),
            ip: ip_str.into(),
            frequency: "5 GHz".into(),
            link_speed: "650 Mbps".into(),
            versions: if active.ip6_config.is_some() {
                "v4+v6".into()
            } else {
                "v4".into()
            },
            throughput: CircularBuffer::new(),
            bssid: "00:00:00:00:00:00".into(),
            gateway,
            dns,
            mac: "00:00:00:00:00:00".into(),
            mtu: 1500,
            security: "WPA2".into(),
            channel: 0,
            autoconnect: true,
            metered: false,
            uptime: Duration::from_secs(0),
            rx_bytes: 0,
            tx_bytes: 0,
            signal_history: CircularBuffer::new(),
            rx_throughput: CircularBuffer::new(),
            tx_throughput: CircularBuffer::new(),
        },
        crate::backend::types::ConnectionType::Ethernet => ConnectionData::WiredActive {
            interface: active.interface.clone().into(),
            ip: ip_str.into(),
            name: active.name.clone().into(),
            versions: if active.ip6_config.is_some() {
                "v4+v6".into()
            } else {
                "v4".into()
            },
            throughput: CircularBuffer::new(),
            gateway,
            dns,
            mac: "00:00:00:00:00:00".into(),
            mtu: 1500,
            link_speed: "1000 Mbps".into(),
            autoconnect: true,
            uptime: Duration::from_secs(0),
            rx_bytes: 0,
            tx_bytes: 0,
            rx_throughput: CircularBuffer::new(),
            tx_throughput: CircularBuffer::new(),
        },
        _ => ConnectionData::WifiInactive {
            name: active.name.clone().into(),
            last_used: Duration::from_secs(0),
            autoconnect: true,
            metered: false,
            tag: "Unknown".into(),
            security: "Unknown".into(),
            bssid: None,
            saved_ip_method: "auto".into(),
            saved_dns: Vec::new(),
        },
    }
}
```

- [ ] **Step 5: Add `ap_to_available` mapper**

```rust
fn ap_to_available(
    ap: &crate::backend::types::AccessPoint,
    saved_ssids: &std::collections::HashSet<Vec<u8>>,
) -> AvailableAPDetails {
    let ssid_str = String::from_utf8_lossy(&ap.ssid);
    let freq_ghz = ap.frequency as f32 / 1000.0;
    let freq_str = format!("{:.2} GHz", freq_ghz);
    let speed_mbps = ap.max_bitrate / 1000;
    let speed_str = format!("{} Mbps", speed_mbps);

    let security_str = match &ap.security {
        crate::backend::types::SecurityType::Open => "Open",
        crate::backend::types::SecurityType::Wep => "WEP",
        crate::backend::types::SecurityType::WpaPsk { version } => match version {
            crate::backend::types::WpaVersion::Wpa1 => "WPA",
            crate::backend::types::WpaVersion::Wpa2 => "WPA2",
            crate::backend::types::WpaVersion::Wpa3 => "WPA3",
        },
        crate::backend::types::SecurityType::WpaEnterprise { version } => match version {
            crate::backend::types::WpaVersion::Wpa1 => "WPA-E",
            crate::backend::types::WpaVersion::Wpa2 => "WPA2-E",
            crate::backend::types::WpaVersion::Wpa3 => "WPA3-E",
        },
    };

    AvailableAPDetails {
        strength: ap.strength as f32,
        ssid: ssid_str.into(),
        security: security_str.into(),
        frequency: freq_str.into(),
        channel: ap.channel,
        link_speed: speed_str.into(),
        bssid: ap.bssid.clone().into(),
        saved: saved_ssids.contains(&ap.ssid),
        signal_history: CircularBuffer::new(),
    }
}
```

### Part C: BridgeResult handlers

- [ ] **Step 6: Add `BridgeResult` match arms in `HomeData::update`**

In the `Component for HomeData` `update` method, add after existing `Message` matches:

```rust
// Inside match ev { ... }
Message::BridgeResult(result) => match result {
    crate::backend::bridge::BridgeResult::DevicesLoaded(Ok(devices)) => {
        self.devices = devices;
    }
    crate::backend::bridge::BridgeResult::DevicesLoaded(Err(e)) => {
        log::warn!("Failed to load devices: {}", e);
    }
    crate::backend::bridge::BridgeResult::ConnectionsLoaded(Ok(profiles)) => {
        self.profiles = profiles;
        self.rebuild_connected_list();
    }
    crate::backend::bridge::BridgeResult::ConnectionsLoaded(Err(e)) => {
        log::warn!("Failed to load connections: {}", e);
    }
    crate::backend::bridge::BridgeResult::ActiveConnectionsLoaded(Ok(actives)) => {
        self.active_connections = actives;
        self.rebuild_connected_list();
    }
    crate::backend::bridge::BridgeResult::ActiveConnectionsLoaded(Err(e)) => {
        log::warn!("Failed to load active connections: {}", e);
    }
    crate::backend::bridge::BridgeResult::AccessPointsLoaded(Ok(aps)) => {
        let saved_ssids: std::collections::HashSet<Vec<u8>> = self
            .profiles
            .iter()
            .filter(|p| matches!(p.connection_type, crate::backend::types::ConnectionType::Wifi))
            .map(|p| p.name.as_bytes().to_vec())
            .collect();
        self.available.list = aps
            .iter()
            .map(|ap| Arc::new(ap_to_available(ap, &saved_ssids)))
            .collect();
    }
    crate::backend::bridge::BridgeResult::AccessPointsLoaded(Err(e)) => {
        log::warn!("Failed to load access points: {}", e);
    }
    crate::backend::bridge::BridgeResult::ScanCompleted { interface, result } => {
        match result {
            Ok(()) => {
                ctx.message
                    .send(Message::RequestScan { interface })
                    .unwrap();
            }
            Err(e) => log::warn!("Scan failed: {}", e),
        }
    }
    crate::backend::bridge::BridgeResult::ConnectionActivated { uuid, result } => {
        match result {
            Ok(active) => {
                self.active_connections.push(active);
                self.rebuild_connected_list();
            }
            Err(e) => log::warn!("Connect failed: {}", e),
        }
    }
    crate::backend::bridge::BridgeResult::ConnectionDeactivated { active_id, result } => {
        match result {
            Ok(()) => {
                self.active_connections.retain(|a| a.id != active_id);
                self.rebuild_connected_list();
            }
            Err(e) => log::warn!("Disconnect failed: {}", e),
        }
    }
    _ => {}
},
```

- [ ] **Step 7: Add `rebuild_connected_list` helper**

```rust
fn rebuild_connected_list(&mut self) {
    let mut list: Vec<Arc<ConnectionData>> = Vec::new();

    // Active connections first
    for active in &self.active_connections {
        list.push(Arc::new(active_to_connection_data(active)));
    }

    // Then inactive profiles
    let active_uuids: std::collections::HashSet<uuid::Uuid> =
        self.active_connections.iter().map(|a| a.profile_uuid).collect();

    for profile in &self.profiles {
        if !active_uuids.contains(&profile.uuid) {
            list.push(Arc::new(profile_to_inactive_data(profile)));
        }
    }

    self.connected.list = list;
}
```

### Part D: User action wiring

- [ ] **Step 8: Update Enter handler for available list to send intent messages**

Replace the existing Enter handler in `HomeData::update`:

```rust
ratatui::crossterm::event::Event::Key(key_event)
    if key_event.code.is_enter() =>
{
    if let Some(s) = self.selected.available_selected() {
        let ap = &self.available.list[s];
        // Find matching profile by SSID
        let ssid_bytes = ap.ssid.as_bytes().to_vec();
        let matching = self.profiles.iter().find(|p| {
            matches!(p.connection_type, crate::backend::types::ConnectionType::Wifi)
                && p.name.as_bytes() == ssid_bytes.as_slice()
        });

        if let Some(profile) = matching {
            // Saved profile: activate directly
            ctx.message
                .send(Message::RequestConnect {
                    uuid: profile.uuid,
                    interface: self
                        .devices
                        .iter()
                        .find(|d| matches!(d.device_type, crate::backend::types::DeviceType::Wifi))
                        .map(|d| d.interface.clone()),
                })
                .unwrap();
        } else {
            // New AP: show auth overlay
            if needs_authentication(ap).is_ok_and(|n| n) {
                ctx.message
                    .send(Message::Authenticate(
                        super::component::AuthenticationData {
                            access_point: ap.clone(),
                        },
                    ))
                    .unwrap();
            }
        }
    }
}
```

Wait, the existing code checks `needs_authentication` first. For unsaved APs, we should always show auth. For saved ones, we activate directly. Let me fix:

```rust
if let Some(s) = self.selected.available_selected() {
    let ap = &self.available.list[s];
    let ssid_bytes = ap.ssid.as_bytes().to_vec();
    let matching = self.profiles.iter().find(|p| {
        matches!(p.connection_type, crate::backend::types::ConnectionType::Wifi)
            && p.name.as_bytes() == ssid_bytes.as_slice()
    });

    if let Some(profile) = matching {
        // Saved profile: activate
        ctx.message
            .send(Message::RequestConnect {
                uuid: profile.uuid,
                interface: self
                    .devices
                    .iter()
                    .find(|d| matches!(d.device_type, crate::backend::types::DeviceType::Wifi))
                    .map(|d| d.interface.clone()),
            })
            .unwrap();
    } else if needs_authentication(ap).is_ok_and(|n| n) {
        // New AP: authenticate first
        ctx.message
            .send(Message::Authenticate(
                super::component::AuthenticationData {
                    access_point: ap.clone(),
                },
            ))
            .unwrap();
    }
}
```

- [ ] **Step 9: Update Enter handler for connected list to send disconnect**

Replace the empty Enter handler for connected list (there isn't one currently — Enter on connected does nothing). Add:

```rust
// In the existing match block for Enter, already handled for available.
// Add a branch for connected list:
if let Some(idx) = self.selected.connected_selected() {
    if idx < self.connected.list.len() {
        // Find the active connection to disconnect
        if let Some(active) = self.active_connections.get(idx) {
            ctx.message
                .send(Message::RequestDisconnect {
                    active_id: active.id.clone(),
                })
                .unwrap();
        }
    }
}
```

Actually, the Enter handler currently only checks `available_selected`. We need to also handle `connected_selected`. The existing code:

```rust
ratatui::crossterm::event::Event::Key(key_event)
    if key_event.code.is_enter() =>
{
    if let Some(s) = self.selected.available_selected()
        && needs_authentication(&self.available.list[s]).is_ok_and(|n| n)
    {
        ctx.message
            .send(Message::Authenticate(...))
            .unwrap();
    }
}
```

We should change it to:
```rust
if key_event.code.is_enter() => {
    if let Some(s) = self.selected.available_selected() {
        // ... handle available connect
    } else if let Some(s) = self.selected.connected_selected() {
        // handle disconnect
        if s < self.active_connections.len() {
            let active = &self.active_connections[s];
            ctx.message
                .send(Message::RequestDisconnect {
                    active_id: active.id.clone(),
                })
                .unwrap();
        }
    }
}
```

- [ ] **Step 10: Update `s` key handler to trigger scan**

Add in the crossterm key handler match:
```rust
ratatui::crossterm::event::Event::Key(key_event)
    if key_event.code.is_char('s') =>
{
    if let Some(dev) = self.devices.iter().find(|d| {
        matches!(d.device_type, crate::backend::types::DeviceType::Wifi)
    }) {
        ctx.message
            .send(Message::RequestScan {
                interface: dev.interface.clone(),
            })
            .unwrap();
    }
}
```

- [ ] **Step 11: Handle `AuthSubmitted` in `HomeData::update`**

Add match arm:
```rust
Message::AuthSubmitted { password, autoconnect } => {
    self.authenticate = None;
    if let Some(s) = self.selected.available_selected() {
        let ap = &self.available.list[s];
        let ssid_bytes = ap.ssid.as_bytes().to_vec();
        let settings = crate::backend::types::ConnectionSettings {
            common: crate::backend::types::CommonSettings {
                name: ap.ssid.to_string(),
                uuid: uuid::Uuid::new_v4(),
                autoconnect,
                interface_name: None,
                cloned_mac: None,
                mtu: None,
                metered: None,
                zone: None,
            },
            connection_type: crate::backend::types::ConnectionTypeSettings::Wifi {
                ssid: ssid_bytes.clone(),
                mode: crate::backend::types::WifiMode::Infra,
                bssid: None,
                band: None,
                channel: None,
                security: crate::backend::types::WifiSecuritySettings::WpaPsk {
                    version: crate::backend::types::WpaVersion::Wpa2,
                    password,
                },
                hidden: false,
                powersave: None,
            },
            ipv4: crate::backend::types::IpSettings {
                method: crate::backend::types::IpMethod::Auto,
                addresses: Vec::new(),
                gateway: None,
                dns: Vec::new(),
                search_domains: Vec::new(),
                routes: Vec::new(),
                never_default: false,
                ignore_auto_dns: false,
            },
            ipv6: crate::backend::types::IpSettings {
                method: crate::backend::types::IpMethod::Auto,
                addresses: Vec::new(),
                gateway: None,
                dns: Vec::new(),
                search_domains: Vec::new(),
                routes: Vec::new(),
                never_default: false,
                ignore_auto_dns: false,
            },
        };
        ctx.message
            .send(Message::RequestAddAndActivate {
                settings,
                interface: self
                    .devices
                    .iter()
                    .find(|d| matches!(d.device_type, crate::backend::types::DeviceType::Wifi))
                    .map(|d| d.interface.clone()),
            })
            .unwrap();
    }
}
```

- [ ] **Step 12: Verify compilation**

Run: `cargo check --features mock`
Expected: PASS or minor errors to fix (unused imports, etc.)

- [ ] **Step 13: Commit**

```bash
git add src/application/home.rs
git commit -m "feat: HomeData handles BridgeResult and rebuilds from backend data"
```

---

## Task 7: Update Authenticate Overlay

**Files:**
- Modify: `src/application/home/authenticate.rs`

- [ ] **Step 1: Change Enter handler to return `AuthSubmitted`**

Replace:
```rust
Message::Crossterm(ratatui::crossterm::event::Event::Key(event))
    if event.code.is_enter() =>
{
    // TODO actually do the connect before finishing
    return Bubble::Yes(Message::FinishAuthentication);
}
```

With:
```rust
Message::Crossterm(ratatui::crossterm::event::Event::Key(event))
    if event.code.is_enter() =>
{
    return Bubble::Yes(Message::AuthSubmitted {
        password: self.password.to_string(),
        autoconnect: self.autoconnect,
    });
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features mock`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/application/home/authenticate.rs
git commit -m "feat: Authenticate returns AuthSubmitted with password"
```

---

## Task 8: Wire main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Import backend and create channel/bridge before Application**

Replace the application creation block:

```rust
use crate::backend::mock::MockBackend;
use crate::backend::bridge::BackendBridge;

fn main() -> anyhow::Result<()> {
    // ... existing setup_logging ...

    let backend = MockBackend::new();

    let (ui_tx, ui_rx) = std::sync::mpsc::channel::<crate::application::component::Message>();
    let ui_tx = std::sync::Arc::new(ui_tx);

    let bridge = std::sync::Arc::new(BackendBridge::new(backend, ui_tx.clone()));

    let context = AppContext {
        log_file,
        fps: 144,
        connection_maxitem: 5,
    };

    let application = Application::new(context, bridge, ui_tx, ui_rx);
    application.run()?;

    Ok(())
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features mock`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire BackendBridge into Application startup"
```

---

## Task 9: Full Verification

- [ ] **Step 1: Run full test suite**

```bash
cargo test --features mock
```
Expected: All existing tests pass, any new tests added pass.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --features mock -- -D warnings
```
Expected: No new warnings.

- [ ] **Step 3: Run binary smoke test**

```bash
cargo run --features mock -- --log-file /tmp/patina-test.log
```
Expected: App starts, shows loading spinner, then displays connection list and access points. Press `q` to quit.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete backend integration via BackendBridge"
```

---

## Self-Review

### Spec Coverage Check

| Spec Section | Task |
|-------------|------|
| Bridge protocol types (§4) | Task 1 |
| Bridge coordinator (§3) | Task 4 |
| Message enum extension (§6.3) | Task 3 |
| Application owns bridge (§6.4) | Task 5 |
| HomeData removes mock, adds handlers (§6.5) | Task 6 |
| Startup sequence (§5.1) | Task 5 + Task 8 |
| Auth flow (§5.2) | Task 6 + Task 7 |
| Disconnect flow (§5.3) | Task 6 |
| Subscription lifecycle (§5.4) | Task 4 |
| Error handling (§7) | Task 4 (logging), Task 6 (warn on errors) |
| Loading state (§8) | Task 5 (`startup_pending`) |
| main.rs wiring (§6.6) | Task 8 |

### Placeholder Scan

- No "TBD" or "TODO" in plan steps.
- No "implement later" or "fill in details".
- All match arms in bridge coordinator are fully written out.
- All mapper functions have complete field assignments.

### Type Consistency Check

- `BridgeCommand::ActivateConnection { uuid: Uuid, interface: Option<String> }` matches backend trait signature `activate_connection(&self, uuid: &Uuid, interface: Option<&str>)` — child task passes `interface.as_deref()`.
- `BridgeResult::ConnectionActivated { uuid: Uuid, result: Result<ActiveConnection> }` — uuid is the requested UUID, returned for correlation.
- `Message::AuthSubmitted { password: String, autoconnect: bool }` is consumed by `HomeData` which constructs `ConnectionSettings`.
- `HomeData::new()` has no parameters now — called from `Application::new` without `&RichContext`.
- `Application::new` takes `(Context, Arc<BackendBridge>, Arc<Sender<Message>>, Receiver<Message>)`.

All type signatures are consistent end-to-end.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-05-21-nm-backend-integration.md`.**

**Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using `executing-plans`, batch execution with checkpoints.

Which approach?
