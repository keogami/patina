# Throughput Collector — Design Spec

> Date: 2026-05-21  
> Scope: Per-connection Rx/Tx throughput: bridge-side batch polling, client-side EMA smoothing, idle decay. Home list shows Tx sparkline; info overlay (`i`) shows both Rx and Tx.

---

## 1. Goals

- Show a live Tx throughput sparkline on each active connection row in the home list.
- Press `i` on a connection to open the info overlay, which shows separate Rx and Tx sparklines.
- Refresh rate: 500 ms.
- Gracefully handle idle interfaces, driver staleness, and backend errors.
- Keep the backend trait unchanged — no new subscription methods.

---

## 2. Success Criteria

- Tx sparkline on home list updates every 500 ms with smoothed throughput.
- After 1 s of no data, sparkline decays to zero.
- Info overlay renders both Rx and Tx sparklines live.
- Mock backend returns realistic incremental counters so the feature is testable without real NM.
- No extra allocations on the hot path.

---

## 3. Architecture

### 3.1 High-level

```
Application (sync thread)
  │
  │ GlobalTick @ FPS
  │ (gate to 500 ms)
  ▼
BridgeCommand::PollStats { interfaces }
  │
  ▼
BackendBridge (tokio)
  │
  │ single child task
  │ calls get_device_stats(iface) for all in parallel
  ▼
BridgeResult::DeviceStats(Result<DeviceStats>) × N
  │
  ▼
HomeData::update → find item by interface → mutate buffers in-place
```

### 3.2 Why batching?

Per-interface one-shots would spawn N child tasks per tick. `PollStats` spawns one child task that fans out to N backend calls in parallel via `futures::future::join_all`, then fans results back as individual `BridgeResult`s. The cost is reduced from N bridge tasks to 1.

---

## 4. Bridge Protocol Additions

### 4.1 New command

```rust
PollStats { interfaces: Vec<String> }
```

Sent by `Application` every 500 ms if there is at least one active connection.

### 4.2 Existing result reused

```rust
DeviceStats(anyhow::Result<DeviceStats>)
```

Already defined in `BridgeResult`. No new variant needed.

---

## 5. Data Model Changes

### 5.1 `Connected.list`

```rust
// Before
pub struct Connected {
    pub list: Vec<Arc<ConnectionData>>,
}

// After
pub struct Connected {
    pub list: Vec<Rc<RefCell<ConnectionData>>>,
}
```

Rationale: throughput buffers are mutated in-place by `HomeData::update`. `Rc<RefCell>` avoids cloning the entire struct on every update or info overlay open. The single-threaded event loop guarantees no contention.

### 5.2 `InfoTarget::Connection`

```rust
// Before
Connection(Arc<ConnectionData>)

// After
Connection(Rc<RefCell<ConnectionData>>)
```

This lets the info overlay render live-updating sparklines without copying data.

### 5.3 Delta-carrying fields in `ConnectionData`

Extend both `WifiActive` and `WiredActive` variants with two new fields:

```rust
last_stats_time: Instant,
prev_rx_bytes: u64,
prev_tx_bytes: u64,
ema_rx: f64,
ema_tx: f64,
```

Initialize:
- `last_stats_time = Instant::now()` at creation time
- `prev_rx_bytes = 0`, `prev_tx_bytes = 0`
- `ema_rx = 0.0`, `ema_tx = 0.0`

These fields are private to the throughput logic; no other code reads them.

---

## 6. Polling Trigger (Application)

### 6.1 New field

```rust
last_stats_poll: Instant,
```

### 6.2 Gate on `GlobalTick`

```rust
const STATS_INTERVAL: Duration = Duration::from_millis(500);

if now - self.last_stats_poll >= STATS_INTERVAL {
    // gather active interfaces from connected list
    // fire BridgeCommand::PollStats
    self.last_stats_poll = now;
}
```

Only fire if `connected.list` is non-empty.

---

## 7. Delta Computation & Smoothing

### 7.1 On `BridgeResult::DeviceStats(Ok(stats))`

1. Find item in `connected.list` where `interface == stats.device`.
2. `let mut item = item.borrow_mut();`
3. Extract current counters from `stats.rx_bytes`, `stats.tx_bytes`.
4. Compute deltas:
   ```rust
   let dt = now - item.last_stats_time;
   let delta_rx = stats.rx_bytes.saturating_sub(item.prev_rx_bytes);
   let delta_tx = stats.tx_bytes.saturating_sub(item.prev_tx_bytes);
   ```
5. If `dt == 0`, skip this sample (avoid div-by-zero).
6. Convert to bits per second:
   ```rust
   let rx_bps = delta_rx as f64 * 8.0 / dt.as_secs_f64();
   let tx_bps = delta_tx as f64 * 8.0 / dt.as_secs_f64();
   ```
7. EMA update (α = 2/(5+1) ≈ 0.333):
   ```rust
   const ALPHA: f64 = 2.0 / 6.0;
   item.ema_rx = if item.ema_rx == 0.0 { rx_bps } else { ALPHA * rx_bps + (1.0 - ALPHA) * item.ema_rx };
   item.ema_tx = if item.ema_tx == 0.0 { tx_bps } else { ALPHA * tx_bps + (1.0 - ALPHA) * item.ema_tx };
   ```
8. Push into circular buffers (clamp to `usize`):
   ```rust
   item.throughput.push(item.ema_tx as usize);
   item.rx_throughput.push(item.ema_rx as usize);
   item.tx_throughput.push(item.ema_tx as usize);
   ```
9. Store new state:
   ```rust
   item.prev_rx_bytes = stats.rx_bytes;
   item.prev_tx_bytes = stats.tx_bytes;
   item.last_stats_time = now;
   ```

### 7.2 First-sample special case

When `prev_rx_bytes == 0 && prev_tx_bytes == 0` on a freshly created item, the first deltas will be large (cumulative counters). Skip pushing into buffers on the first sample — just store counters and time. Start EMA on the second sample.

---

## 8. Idle Decay

### 8.1 On `GlobalTick`

For each item in `connected.list`:

```rust
if now - item.last_stats_time > Duration::from_secs(1) {
    item.ema_rx = 0.0;
    item.ema_tx = 0.0;
    item.throughput.push(0);
    item.rx_throughput.push(0);
    item.tx_throughput.push(0);
    item.last_stats_time = now; // reset to avoid re-pushing zero every tick
}
```

This handles:
- NM not signaling idle interfaces (counters stuck)
- Backend temporarily unavailable for one interface
- Connection deactivating but still in the list briefly

---

## 9. Error Handling

- **Backend error on `PollStats`:** one `BridgeResult::DeviceStats(Err(e))` per failed interface. `HomeData` logs via `log::warn!`. The affected interface naturally decays to zero via idle decay.
- **Interface not found in list:** log warning, drop the result.
- **Backend does not support stats:** decay to zero. No crash.

---

## 10. Rendering

### 10.1 Home list (no change to layout)

`ConnectionItem` already reads `throughput` for the sparkline. It now contains the Tx EMA instead of empty data. No visual changes required.

### 10.2 Info overlay

The `Info` widget for active connections renders:
- Two sparklines: `rx_throughput` and `tx_throughput`
- Labels: "Rx" and "Tx"
- A numeric readout alongside each sparkline (e.g. "3.2 MiB/s")

Use the existing `BrailleSparkline` widget. Place Rx and Tx side by side if width permits; stack vertically if narrow.

---

## 11. Mock Backend Changes

`MockBackend::get_device_stats` currently returns a static snapshot. Change it to:

1. Store per-interface counters internally (`Mutex<HashMap<String, (u64, u64)>>`).
2. On each call, increment both counters by a small randomized amount.
3. Return the updated counters.

This produces visible, continuously growing sparklines for testing.

---

## 12. Files to Touch

| File | Change |
|------|--------|
| `src/backend/bridge.rs` | Add `BridgeCommand::PollStats` |
| `src/backend/mock.rs` | Make `get_device_stats` return incremental counters |
| `src/application/component.rs` | Change `InfoTarget::Connection` to `Rc<RefCell<ConnectionData>>` |
| `src/application/mod.rs` | Add `last_stats_poll`, gate `GlobalTick` to fire `PollStats` |
| `src/application/home.rs` | Change `Connected.list` to `Rc<RefCell<ConnectionData>>`; add delta/EMA/idle-decay logic in `update` |
| `src/application/home/connected.rs` | Update list type from `&[Arc<ConnectionData>]` to `&[Rc<RefCell<ConnectionData>>]`; `borrow()` in render |
| `src/application/home/info.rs` | Accept `Rc<RefCell<ConnectionData>>`; render Rx+Tx sparklines |

---

## 13. Deferred

| Feature | Why Deferred |
|---------|-------------|
| Plugin VPN `/proc/net/dev` fallback | Requires IP→interface resolution; VPN subsystem not yet wired |
| Bond/bridge slave aggregation | Multi-device connections need an aggregation strategy |
| Real NM `PropertiesChanged` push | Requires `ZbusBackend`; trait would need a stats subscription |
| Sparkline toggle (Rx/Tx on home list) | Low priority; `i` overlay covers detail view |
| Human-readable bps formatting in info overlay | Small UI polish; can be added after stats flow works |

---

## 14. Spec Self-Review

- **Placeholder scan:** None. `get_device_stats` trait signature already exists. `DeviceStats` struct already contains counters.
- **Internal consistency:** Batch command spawns one bridge task; idle decay runs on `GlobalTick`; EMA parameters match stated 5-sample window. No contradictions.
- **Scope check:** This is a single focused spec. Stats collection + display only. No VPN, no bond aggregation, no new subscription protocol.
- **Ambiguity check:** First-sample skip is explicit to avoid initial burst. `last_stats_time` reset in idle decay prevents zero-flooding. Epoch for clamping to `usize` is implicit (acceptable; would need ~16 EiB/s to overflow, impossible).

---

*Spec written 2026-05-21. Next step: invoke `writing-plans` skill to produce implementation plan.*
