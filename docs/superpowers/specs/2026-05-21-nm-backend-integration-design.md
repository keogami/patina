# Backend Integration Spec — Bridge Actor Architecture

> Date: 2026-05-21  
> Scope: Wire the async `NetworkManagerBackend` trait into the synchronous message-passing UI event loop. Only covers integration plumbing; real connection/data handling (AP signal polling, throughput) is deferred.

---

## 1. Goals

- Bridge async backend requests/responses and push subscriptions into the existing synchronous `std::sync::mpsc` UI event loop.
- Keep panes backend-agnostic — they react to `Message::BridgeResult(...)` and emit user-intent messages only.
- Centralize all async backend invocation in a single coordinator (spawned as one tokio task) that delegates to detached child tasks.
- Support clean subscription lifecycle (start/stop via `CancellationToken`).

---

## 2. Constraints

- The UI event loop MUST stay synchronous (`ratatui::run` + `std::sync::mpsc`).
- The main thread MUST NOT participate in tokio's worker pool (existing setup already creates a multi-thread runtime entered via `.enter()`, not `.block_on()`).
- Backend trait implementations (`MockBackend`, future `ZbusBackend`) cannot block the coordinator.
- Auth flow is fully pre-collected by the UI before any backend call — no suspended backend callbacks.

---

## 3. Architecture

### 3.1 High-level

```
┌─────────────────┐     std::sync::mpsc     ┌──────────────────┐
│  Application    │ ◄──────────────────────►│  UI Event Loop   │
│  (sync thread)  │    Message::Crossterm   │  (ratatui::run)  │
│                 │    Message::BridgeResult│                  │
└───────┬─────────┘                         └──────────────────┘
        │
        │ BackendBridge::send(cmd)
        │ (tokio::sync::mpsc::Unbounded<BridgeCommand>)
        ▼
┌─────────────────────────┐
│  BackendBridge (tokio)  │  ← single coordinator task
│  ─────────────────────  │
│  • owns backend clone   │
│  • owns ui_tx Arc       │
│  • spawns child tasks   │
│  • subscription registry│
└───────┬─────────────────┘
        │ spawns child with backend.clone(), ui_tx.clone()
        ▼
┌─────────────────────────┐         ┌──────────────────────┐
│  Child Task (tokio)     │◄───────►│  NetworkManager      │
│  ────────────────────   │         │  Backend impl        │
│  future-based call      │         │  (Mock / Zbus)       │
│  send result → ui_tx    │         └──────────────────────┘
└─────────────────────────┘
```

### 3.2 Why a coordinator + child tasks?

A single sequential bridge task would bottleneck on blocking calls (e.g., `request_scan`, NM D-Bus I/O). Instead, the coordinator is purely a dispatcher: for each `BridgeCommand`, it spawns a short-lived detached child tokio task. Child tasks call the backend and fire results back into the sync mpsc channel. Subscriptions are also child tasks, but long-lived and cancellable.

---

## 4. Bridge Protocol

### 4.1 Commands (sync thread → bridge)

`BridgeCommand` is an enum of all possible backend interactions. The UI thread never calls backend methods directly.

**One-shot commands:**
- Discovery: `ListDevices`, `ListAccessPoints { interface }`, `RequestScan { interface }`
- Connection Control: `ListConnections`, `GetActiveConnections`, `ActivateConnection { uuid, interface }`, `DeactivateConnection { active_id }`, `DisconnectDevice { interface }`, `AddAndActivateConnection { settings, interface }`
- Configuration: `GetSettings { uuid }`, `UpdateConnection { uuid, settings }`, `AddConnection { settings }`, `DeleteConnection { uuid }`, `GetSecrets { uuid, setting_name }`
- Global State: `GetGlobalState`, `SetNetworkingEnabled(bool)`, `SetWirelessEnabled(bool)`, `SetWwanEnabled(bool)`, `SaveHostname(String)`, `SetDeviceManaged { interface, managed }`, `SetDeviceAutoconnect { interface, autoconnect }`
- Stats: `GetDeviceStats { interface }`

**Subscription commands:**
- `SubscribeDeviceEvents` → returns `SubscriptionHandle`
- `SubscribeApEvents { interface }` → returns `SubscriptionHandle`
- `SubscribeProfileEvents` → returns `SubscriptionHandle`
- `SubscribeActiveEvents` → returns `SubscriptionHandle`
- `Unsubscribe { handle }`

### 4.2 Results (bridge → sync thread)

`BridgeResult` is an enum of all possible backend responses. All variants are wrapped in `Message::BridgeResult(BridgeResult)` and flow through the existing mpsc channel.

**One-shot results:**
- `DevicesLoaded(Result<Vec<Device>>)`
- `AccessPointsLoaded(Result<Vec<AccessPoint>>)`
- `ConnectionsLoaded(Result<Vec<ConnectionProfile>>)`
- `ActiveConnectionsLoaded(Result<Vec<ActiveConnection>>)`
- `GlobalStateLoaded(Result<GlobalState>>)`
- `ScanCompleted { interface: String, result: Result<()> }`
- `ConnectionActivated { uuid: Uuid, result: Result<ActiveConnection> }`
- `ConnectionDeactivated { active_id: String, result: Result<()> }`
- `DeviceDisconnected { interface: String, result: Result<()> }`
- `SettingsLoaded { uuid: Uuid, result: Result<ConnectionSettings> }`
- `ConnectionUpdated { uuid: Uuid, result: Result<()> }`
- `ConnectionAdded { result: Result<ConnectionProfile> }`
- `ConnectionDeleted { uuid: Uuid, result: Result<()> }`
- `SecretsLoaded { uuid: Uuid, result: Result<Secrets> }`
- `DeviceStatsLoaded { interface: String, result: Result<DeviceStats> }`
- `SubscriptionStarted { handle: SubscriptionHandle }`

**Push events (continuously streamed by subscription tasks):**
- `DeviceEvent(DeviceEvent)`
- `ApEvent(ApEvent)`
- `ProfileEvent(ProfileEvent)`
- `ActiveEvent(ActiveEvent)`

### 4.3 `SubscriptionHandle`

An opaque newtype (`u64` internally). Used only for `Unsubscribe`. The bridge tracks `HashMap<SubscriptionHandle, CancellationToken>`. Dropping the token signals the child task to exit via `tokio::select!`.

---

## 5. Data Flow Scenarios

### 5.1 Startup Sequence

1. `main.rs` creates `Backend`, creates `BackendBridge(backend, ui_tx)`.
2. `Application::new(context, bridge)` stores `Arc<BackendBridge>`.
3. Inside `Application::run`, immediately before entering the message loop:
   - `bridge.send(ListDevices)`
   - `bridge.send(ListConnections)`
   - `bridge.send(GetActiveConnections)`
4. `Application` increments `pending_ops`.
5. Bridge spawns 3 child tasks → results arrive as `BridgeResult`.
6. `HomeData` matches `DevicesLoaded`, populates internal device list.
7. `Application` intercepts `Message::BridgeResult(DevicesLoaded(Ok(devices)))`, detects WiFi device(s), fires `bridge.send(ListAccessPoints { interface })`.
8. `AccessPointsLoaded` arrives → `HomeData` populates available list.
9. `Application` decrements `pending_ops` to 0 → sends `Message::FinishLoading`.
10. `HomeData` clears spinner.

### 5.2 User Connects to an AP

1. User presses `Enter` on an AP in `AvailableList`.
2. `HomeData::update` matches `Crossterm(Key(Enter))` → checks `needs_authentication(ap)`. If true:
   - Opens `Authenticate` overlay with `AuthenticationData { access_point }`.
   - Returns `Bubble::No`.
3. User types password + presses `Enter`.
4. `Authenticate::update` produces `Bubble::Yes(Message::SubmitAuth { settings, interface })` where `settings` contains the password in `WifiSecuritySettings::WpaPsk`.
5. `Application::update` intercepts `SubmitAuth`:
   - `bridge.send(AddAndActivateConnection { settings, interface })`
   - Increments `pending_ops`.
6. Bridge child task calls backend.
7. Result arrives as `BridgeResult::ConnectionActivated` → `HomeData` updates connected list.
8. `Application` decrements `pending_ops`.

### 5.3 Disconnecting an Active Connection

1. User presses `Enter` on an item in `ConnectedList`.
2. `HomeData::update` matches `Crossterm(Key(Enter))` + connected list active → sends `Message::RequestDisconnect { active_id }`.
3. `Application::update` intercepts → `bridge.send(DeactivateConnection { active_id })`.
4. Result arrives → `HomeData` refreshes or removes the item.

### 5.4 Subscription Lifecycle

1. After `FinishLoading`, `Application` fires `SubscribeDeviceEvents`, `SubscribeApEvents { interface }`, `SubscribeProfileEvents`, `SubscribeActiveEvents`.
2. Bridge returns `SubscriptionStarted { handle }` for each.
3. Bridge spawns long-lived child tasks per subscription. Each task:
   - Calls the backend subscription method (e.g., `subscribe_ap_events(interface)`)
   - Loops over the returned `mpsc::UnboundedReceiver`
   - Forwards each event as `BridgeResult::ApEvent(...)` via `ui_tx`
   - Uses `tokio::select!` on `recv()` vs `child_token.cancelled()`
4. If user closes the app or disables WiFi, `Application` sends `Unsubscribe { handle }`.
5. Bridge drops the token → child task exits cleanly.

---

## 6. Integration Points by File

### 6.1 `src/backend/bridge.rs` (new)

- Define `BridgeCommand`, `BridgeResult`, `SubscriptionHandle`.
- Define `BackendBridge` struct with `cmd_tx: UnboundedSender<BridgeCommand>`.
- constructor `new<B: NetworkManagerBackend>(backend: B, ui_tx: mpsc::Sender<Message>) → Self`
  - Spawns the coordinator tokio task.
  - coordinator owns `backend`, `ui_tx`, subscription registry (`HashMap<SubscriptionHandle, CancellationToken>`).
  - In `tokio::spawn` loop: `while let Some(cmd) = cmd_rx.recv().await { ... }`
  - Match on `cmd`, spawn child tasks with `backend.clone()` and `ui_tx.clone()`.
- `send(&self, cmd: BridgeCommand)` — fire-and-forget into the unbounded channel.

### 6.2 `src/backend/mod.rs`

- Re-export `bridge::*`, `types::*`, and trait.
- Keep `#[cfg(feature = "mock")] pub mod mock;`.

### 6.3 `src/application/component.rs`

Extend `Message`:

```rust
pub enum Message {
    // Existing
    Crossterm(crossterm::event::Event),
    GlobalTick,
    LoadConnected(Connected),
    LoadAvailable(Available),
    FinishLoading,
    Authenticate(AuthenticationData),
    FinishAuthentication,
    OpenInfo(InfoTarget),
    FinishInfo,

    // NEW
    BridgeResult(BridgeResult),

    // User-intent messages intercepted by Application
    RequestConnect { uuid: Uuid, interface: Option<String> },
    RequestDisconnect { active_id: String },
    RequestScan { interface: String },
    RequestDeviceList,
    RequestConnectionList,
    SubmitAuth { settings: ConnectionSettings, interface: Option<String> },
}
```

No other changes to `Context`, `RichContext`, `Bubble`, or `Component` trait.

### 6.4 `src/application/mod.rs`

- `Application` gains fields:
  - `bridge: Arc<BackendBridge>`
  - `pending_ops: u32`
- `new(context, bridge)` stores both. No pane changes.
- `run()` startup: immediately sends `DeviceList`, `ConnectionList`, `ActiveConnections` via bridge.
- `update()` gains interception logic:
  - Matches `Message::RequestConnect`, `RequestDisconnect`, `RequestScan`, `SubmitAuth` → fires corresponding `BridgeCommand`.
  - Matches `Message::BridgeResult(...)` → the result is forwarded to `self.pane.update()` after `pending_ops` bookkeeping.
  - `pending_ops` incremented on every intercepted user request; decremented on every `BridgeResult`.
  - When `pending_ops` reaches 0 and `loading` is still active, sends `Message::FinishLoading`.

### 6.5 `src/application/home.rs`

- `HomeData::new(...)` no longer contains inline mock data or `tokio::spawn` loading tasks. It initializes empty lists and sets `loading = Some(Spinner)`.
- `Component::update` gains new match arms for `Message::BridgeResult`:
  - `DevicesLoaded(Ok(devices))` → store device list (for AP listing and info overlay)
  - `ConnectionsLoaded(Ok(conns))` → map to `Vec<Arc<ConnectionData>>` via inline mapper
  - `ActiveConnectionsLoaded(Ok(actives))` → map active connections into `ConnectionData::WifiActive` / `WiredActive`
  - `AccessPointsLoaded(Ok(aps))` → map to `Vec<Arc<AvailableAPDetails>>` via inline mapper
  - `ConnectionActivated(...)` / `ConnectionDeactivated(...)` → trigger list refresh
  - `ScanCompleted(...)` → no-op for now (future: trigger `RequestApList`)
- Inline backend→UI mappers defined as private `fn` or `impl` blocks in `home.rs`. These translate `backend::types::*` into `ConnectionData` / `AvailableAPDetails`.
- `Authenticate` overlay interaction: returns `SubmitAuth` with `ConnectionSettings` instead of the current pattern of `FinishAuthentication`.

---

## 7. Error Handling

- Backend errors arrive as `BridgeResult::XxxLoaded(Err(e))`.
- `Application` logs all errors via `log::warn!` or `log::error!`.
- **Discovery / list loads:** transient error. `HomeData` may show a message like `"Failed to load connections"` in place of the empty list.
- **Connect / disconnect / auth:** modal error overlay. `HomeData` shows a dismissible error dialog.
- **Scan:** transient toast (header line or inline message).
- **Subscription errors:** log and drop. The subscription child task exits; `Application` may re-subscribe on next relevant user action.
- `Application` does not crash the app on backend errors — all results are `Result<T>`.

---

## 8. Loading State

- `HomeData.loading: Option<Spinner>` preserved.
- `Application.pending_ops: u32` tracks outstanding backend calls.
- Incremented every time `Application` sends a `BridgeCommand`.
- Decremented every time `Application` receives a `BridgeResult`.
- When `pending_ops` reaches 0 and `loading.is_some()` → `Message::FinishLoading`.
- Subscriptions do NOT increment `pending_ops` — they are background, not part of initial load.

---

## 9. Testing

### 9.1 Bridge tests

- Create a mock backend, create `BackendBridge` over a `std::sync::mpsc` channel.
- Send `ListDevices` → assert `DevicesLoaded` arrives on channel.
- Send `ActivateConnection` with known UUID → assert `ConnectionActivated` arrives with correct ID.
- Start subscription, receive events, send `Unsubscribe`, assert no more events arrive.

### 9.2 Integration tests

- Inject `Message::BridgeResult(DevicesLoaded(Ok(vec![...])))` into `HomeData::update` and assert correct state change.
- Verify `Application::update` correctly intercepts `RequestConnect` and converts to `BridgeCommand`.

### 9.3 HomeData tests

- Test mapper functions: given a `backend::Device` + `backend::ActiveConnection`, verify produced `ConnectionData::WifiActive` has correct fields.
- Test that `FinishLoading` clears spinner.

---

## 10. Deferred

| Feature | Why Deferred |
|---------|-------------|
| AP signal strength polling | Requires dedicated task design for continuous polling + data smoothing. |
| Real-time throughput / device stats streaming | Needs rate-limiting and display smoothing. |
| Full subscription event handling in UI | Subscription types defined; `HomeData` will map them to updates in a follow-up. |
| Checkpoint / rollback | Not part of nmtui parity. |
| Secret Agent async callbacks | Auth is pre-collected by UI overlay before backend call. |
| `ZbusBackend` real implementation | Separate spec; this integration is backend-agnostic. |

---

## 11. Spec Self-Review

- **Placeholder scan:** No TBDs, no vague method signatures. All `BridgeCommand` / `BridgeResult` variants are named. Mappers are described as "inline in home.rs" — bounded enough.
- **Internal consistency:** Startup flow (Section 5.1) correctly shows `Application` firing follow-up `ListAccessPoints` after `DevicesLoaded`, matching the "Application triggers follow-ups, not HomeData" rule. Auth flow (5.2) shows pre-collected password in `ConnectionSettings`, consistent with trait design.
- **Scope check:** This is a single focused spec — integration plumbing only. No real Zbus, no stats streaming, no VPN.
- **Ambiguity check:** `SubscriptionHandle` is opaque — good. `pending_ops` only counts one-shots, not subscriptions — stated explicitly. `Application::update` intercepts certain `Message` variants before passing to pane — clear.

---

*Spec written 2026-05-21. Next step: invoke `writing-plans` skill to produce implementation plan.*
