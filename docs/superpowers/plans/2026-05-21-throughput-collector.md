# Throughput Collector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire per-connection throughput stats into the UI: batch bridge polling every 500ms, client-side EMA smoothing, idle decay to zero. Home list shows Tx sparkline; info overlay shows Rx+Tx.

**Architecture:** The backend integration (bridge, Application intercepts, HomeData rebuilds) is already done. This plan adds: (1) a batched `PollStats` bridge command that fans out `get_device_stats` in parallel, (2) `Rc<RefCell<ConnectionData>>` so HomeData can mutate throughput buffers in-place, (3) Application gates `GlobalTick` to fire `PollStats`, (4) HomeData computes deltas → EMA → push into circular buffers, with idle decay on tick.

**Tech Stack:** Rust 2024, ratatui, crossterm, std mpsc, tokio, circular_buffer.

---

## Files to Touch

| File | Responsibility |
|------|---------------|
| `src/backend/bridge.rs` | Add `BridgeCommand::PollStats`, add batch handler in coordinator |
| `src/application/mod.rs` | Add `last_stats_poll`, gate `GlobalTick` → `PollStats` |
| `src/application/component.rs` | Change `InfoTarget::Connection` to `Rc<RefCell<ConnectionData>>` |
| `src/application/home.rs` | Change `Connected.list` type, add stats delta/EMA/idle_decay logic, handle `DeviceStats` results |
| `src/application/home/connected.rs` | Change list/item types to `Rc<RefCell<ConnectionData>>`, borrow in render |
| `src/application/home/info.rs` | Accept `Rc<RefCell<ConnectionData>>`, render live (no change to render needed since it already reads the buffers) |
| `src/backend/mock.rs` | Make `get_device_stats` return incremental counters |

---

## Task 1: Add `PollStats` to Bridge

**Files:**
- Modify: `src/backend/bridge.rs`

- [ ] **Step 1: Add `PollStats` command variant**

In `BridgeCommand` enum, add:

```rust
PollStats { interfaces: Vec<String> },
```

Place it in the Monitoring section, after `GetDeviceStats`.

- [ ] **Step 2: Add batch handler in bridge coordinator**

In the coordinator's `while let Some(cmd) = cmd_rx.recv().await` loop, add a match arm for `PollStats`:

```rust
BridgeCommand::PollStats { interfaces } => {
    let b = backend.clone();
    let tx = ui_tx.clone();
    tokio::spawn(async move {
        let mut handles = Vec::with_capacity(interfaces.len());
        for iface in &interfaces {
            let b = b.clone();
            let iface = iface.clone();
            handles.push(tokio::spawn(async move {
                (iface.clone(), b.get_device_stats(&iface).await.map_err(BridgeError::from))
            }));
        }
        for handle in handles {
            if let Ok((iface, result)) = handle.await {
                let _ = tx.send(Message::BridgeResult(BridgeResult::DeviceStats(result)));
            }
        }
    });
}
```

This spawns one bridge child task. Inside that task, it spawns sub-tasks per interface and fans results back individually.

- [ ] **Step 3: Verify build**

Run: `cargo check --features mock`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add src/backend/bridge.rs
git commit -m "feat: add PollStats batch command to bridge"
```

---

## Task 2: Extend `ConnectionData` with Stats-Tracking Fields

**Files:**
- Modify: `src/application/home/connected.rs`

- [ ] **Step 1: Add throughput tracking fields to active variants**

In `ConnectionData`, extend `WifiActive` and `WiredActive` with:

```rust
last_stats_time: Instant,
prev_rx_bytes: u64,
prev_tx_bytes: u64,
ema_rx: f64,
ema_tx: f64,
```

Add `use std::time::Instant;` at the top of the file.

Update all constructors (`map_active_to_connection_data` in `home.rs` will do this; the struct itself just needs the fields declared).

- [ ] **Step 2: Verify build**

Run: `cargo check --features mock`
Expected: Compile errors in `home.rs` where `ConnectionData` is constructed — that's expected, we'll fix in Task 3.

- [ ] **Step 3: Commit**

```bash
git add src/application/home/connected.rs
git commit -m "feat: add throughput tracking fields to ConnectionData"
```

---

## Task 3: Change `Arc` → `Rc<RefCell>` in Connected List and InfoTarget

**Files:**
- Modify: `src/application/component.rs`
- Modify: `src/application/home.rs`
- Modify: `src/application/home/connected.rs`
- Modify: `src/application/home/info.rs`

- [ ] **Step 1: Update `Connected` struct in `component.rs`**

```rust
use std::rc::Rc;
use std::cell::RefCell;

pub struct Connected {
    pub list: Vec<Rc<RefCell<ConnectionData>>>,
}
```

Change `InfoTarget::Connection`:
```rust
Connection(Rc<RefCell<ConnectionData>>),
```

- [ ] **Step 2: Update `ConnectedList` in `connected.rs`**

```rust
use std::rc::Rc;
use std::cell::RefCell;

pub struct ConnectedList<'a> {
    pub items: &'a [Rc<RefCell<ConnectionData>>],
    pub selected: Option<usize>,
    pub max_items: usize,
    pub scroll_state: ScrollState,
}
```

In `ConnectedList::render`, the mapping closure needs no change except `item.clone()` instead of just `item` (since `item` is now `&Rc<RefCell<ConnectionData>>`, cloning gives a new `Rc`):

```rust
let items = self.items[range]
    .iter()
    .enumerate()
    .map(|(idx, item)| ConnectionItem {
        data: item.clone(),
        selected: self.selected.is_some_and(|sel| sel == start + idx),
        disabled: self.selected.is_none(),
    });
```

- [ ] **Step 3: Update `ConnectionItem` in `connected.rs`**

```rust
pub struct ConnectionItem {
    pub data: Rc<RefCell<ConnectionData>>,
    pub selected: bool,
    pub disabled: bool,
}
```

In `ConnectionItem::render`, wrap the `match self.data` with a borrow:

```rust
let data = self.data.borrow();
match &*data {
    ConnectionData::WifiActive { ... } => { ... }
    ConnectionData::WiredActive { ... } => { ... }
    ConnectionData::WifiInactive { ... } => { ... }
}
```

Update all field accesses from `self.data.` to `data.` or `(*data).`.

- [ ] **Step 4: Update `home.rs` usages**

In `rebuild_connected_list`, change:
```rust
connected_list.push(Arc::new(map_active_to_connection_data(...)));
// →
connected_list.push(Rc::new(RefCell::new(map_active_to_connection_data(...))));
```

Same for inactive profiles.

In the `i` key handler, change:
```rust
Some(InfoTarget::Connection(self.connected.list[idx].clone()))
// →
Some(InfoTarget::Connection(self.connected.list[idx].clone()))
```

This stays the same because `Rc::clone()` works (the type changed, the expression doesn't need to).

In `OpenInfo` handler, no change needed.

- [ ] **Step 5: Update `info.rs` `InfoTarget` and usages**

Change `InfoTarget::Connection` to hold `Rc<RefCell<ConnectionData>>`.

In `Info::render_title`, `render_subtitle`, `build_sections`, `render_footer`, wrap `c.as_ref()` accesses with `.borrow()`:

```rust
InfoTarget::Connection(c) => match c.borrow().as_ref() { ... }
```

Or more idiomatically, use `let data = c.borrow(); match &*data { ... }`.

- [ ] **Step 6: Verify build**

Run: `cargo check --features mock`
Expected: No errors.

- [ ] **Step 7: Commit**

```bash
git add src/application/component.rs src/application/home.rs src/application/home/connected.rs src/application/home/info.rs
git commit -m "refactor: use Rc<RefCell<ConnectionData>> for in-place mutation"
```

---

## Task 4: Add `last_stats_poll` and `GlobalTick` Gating in Application

**Files:**
- Modify: `src/application/mod.rs`

- [ ] **Step 1: Add fields and constant**

```rust
use std::time::{Duration, Instant};

const STATS_INTERVAL: Duration = Duration::from_millis(500);

pub struct Application {
    // ... existing fields ...
    last_stats_poll: Instant,
}
```

In `Application::new`, initialize:
```rust
last_stats_poll: Instant::now(),
```

- [ ] **Step 2: Add helper to extract active interfaces**

```rust
fn get_active_interfaces(&self) -> Vec<String> {
    match &self.pane {
        Pane::Home(home) => {
            home.active_connections.iter()
                .map(|ac| ac.interface.clone())
                .collect()
        }
    }
}
```

- [ ] **Step 3: Gate `GlobalTick` to fire `PollStats`**

In `Application::update`, in the `match &ev` block, add before the existing arm interception:

```rust
Message::GlobalTick => {
    let now = Instant::now();
    if now.duration_since(self.last_stats_poll) >= STATS_INTERVAL {
        let interfaces = self.get_active_interfaces();
        if !interfaces.is_empty() {
            self.bridge.send(BridgeCommand::PollStats { interfaces });
        }
        self.last_stats_poll = now;
    }
}
```

**Important:** This `GlobalTick` arm should fall through to `self.pane.update(&self.context, ev)` at the end so the pane still gets the tick (for spinner, idle decay). Do NOT `return` here like the other intercepted messages.

- [ ] **Step 4: Verify build**

Run: `cargo check --features mock`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add src/application/mod.rs
git commit -m "feat: gate GlobalTick to fire PollStats every 500ms"
```

---

## Task 5: Add Stats Processing and Idle Decay in HomeData

**Files:**
- Modify: `src/application/home.rs`

- [ ] **Step 1: Add EMA constants and helper**

At the top of `home.rs`:

```rust
use std::time::Instant;

const STATS_EMA_ALPHA: f64 = 2.0 / 6.0; // 5-sample EMA window
const IDLE_DECAY_THRESHOLD: Duration = Duration::from_secs(1);
```

- [ ] **Step 2: Handle `DeviceStats` in `HomeData::update`**

In the `Message::BridgeResult(result)` match arm, add:

```rust
BridgeResult::DeviceStats(Ok(stats)) => {
    // Find the connection by interface name
    for item in &self.connected.list {
        let mut data = item.borrow_mut();
        let interface = match &*data {
            ConnectionData::WifiActive { interface, .. } => interface.as_ref(),
            ConnectionData::WiredActive { interface, .. } => interface.as_ref(),
            ConnectionData::WifiInactive { .. } => continue,
        };
        if interface != stats.device {
            continue;
        }
        let now = Instant::now();
        let dt = now.duration_since(data.last_stats_time());
        
        // Skip if zero delta time
        if dt.is_zero() {
            break;
        }
        
        let delta_rx = stats.rx_bytes.saturating_sub(data.prev_rx_bytes());
        let delta_tx = stats.tx_bytes.saturating_sub(data.prev_tx_bytes());
        
        let rx_bps = delta_rx as f64 * 8.0 / dt.as_secs_f64();
        let tx_bps = delta_tx as f64 * 8.0 / dt.as_secs_f64();
        
        // First-sample skip: if prev_bytes are both 0, just store counters
        let is_first_sample = data.prev_rx_bytes() == 0 && data.prev_tx_bytes() == 0;
        
        if !is_first_sample {
            // EMA update
            data.set_ema_rx(if data.ema_rx() == 0.0 { rx_bps } else { STATS_EMA_ALPHA * rx_bps + (1.0 - STATS_EMA_ALPHA) * data.ema_rx() });
            data.set_ema_tx(if data.ema_tx() == 0.0 { tx_bps } else { STATS_EMA_ALPHA * tx_bps + (1.0 - STATS_EMA_ALPHA) * data.ema_tx() });
            
            // Push into buffers
            data.push_throughput(data.ema_tx() as usize);
            data.push_rx_throughput(data.ema_rx() as usize);
            data.push_tx_throughput(data.ema_tx() as usize);
        }
        
        data.set_prev_rx_bytes(stats.rx_bytes);
        data.set_prev_tx_bytes(stats.tx_bytes);
        data.set_last_stats_time(now);
        break;
    }
}
BridgeResult::DeviceStats(Err(e)) => {
    log::warn!("DeviceStats error: {}", e.message);
}
```

**Wait** — the `ConnectionData` enum doesn't have accessor methods. Let me revise. Rather than adding a bunch of accessor methods, it's cleaner to use direct pattern matching and functional update. But `RefCell::borrow_mut()` gives a mutable reference, and we need to destructure and reconstruct the enum... that's awkward.

Alternative: add helper methods to `ConnectionData` for the fields that need mutation. Or, since we're only mutating the active variants, use `if let`:

```rust
if let ConnectionData::WifiActive { 
    ref mut last_stats_time,
    ref mut prev_rx_bytes,
    ref mut prev_tx_bytes,
    ref mut ema_rx,
    ref mut ema_tx,
    ref mut throughput,
    ref mut rx_throughput,
    ref mut tx_throughput,
    ref interface,
    ..
} = &mut *data {
    if interface.as_ref() == stats.device { ... }
}
```

Yes, this works. Pattern match with `ref mut` bindings. Let me rewrite the step more concretely.

Actually, since we need to match both `WifiActive` and `WiredActive`, let's write it with a helper function or just duplicate the logic. Since the field names overlap in both variants, we can use a macro or just inline both matches. For clarity, inline both:

```rust
for item in &self.connected.list {
    let mut data = item.borrow_mut();
    let matched = match &mut *data {
        ConnectionData::WifiActive {
            ref interface,
            ref mut last_stats_time,
            ref mut prev_rx_bytes,
            ref mut prev_tx_bytes,
            ref mut ema_rx,
            ref mut ema_tx,
            ref mut throughput,
            ref mut rx_throughput,
            ref mut tx_throughput,
            ..
        } if interface.as_ref() == stats.device => {
            update_stats(
                last_stats_time, prev_rx_bytes, prev_tx_bytes,
                ema_rx, ema_tx, throughput, rx_throughput, tx_throughput,
                &stats,
            );
            true
        }
        ConnectionData::WiredActive {
            ref interface,
            ref mut last_stats_time,
            ref mut prev_rx_bytes,
            ref mut prev_tx_bytes,
            ref mut ema_rx,
            ref mut ema_tx,
            ref mut throughput,
            ref mut rx_throughput,
            ref mut tx_throughput,
            ..
        } if interface.as_ref() == stats.device => {
            update_stats(
                last_stats_time, prev_rx_bytes, prev_tx_bytes,
                ema_rx, ema_tx, throughput, rx_throughput, tx_throughput,
                &stats,
            );
            true
        }
        _ => false,
    };
    if matched {
        break;
    }
}
```

With a helper function:

```rust
fn update_stats(
    last_stats_time: &mut Instant,
    prev_rx_bytes: &mut u64,
    prev_tx_bytes: &mut u64,
    ema_rx: &mut f64,
    ema_tx: &mut f64,
    throughput: &mut CircularBuffer<48, usize>,
    rx_throughput: &mut CircularBuffer<48, usize>,
    tx_throughput: &mut CircularBuffer<48, usize>,
    stats: &DeviceStats,
) {
    let now = Instant::now();
    let dt = now.duration_since(*last_stats_time);
    
    if dt.is_zero() {
        return;
    }
    
    let delta_rx = stats.rx_bytes.saturating_sub(*prev_rx_bytes);
    let delta_tx = stats.tx_bytes.saturating_sub(*prev_tx_bytes);
    
    let rx_bps = delta_rx as f64 * 8.0 / dt.as_secs_f64();
    let tx_bps = delta_tx as f64 * 8.0 / dt.as_secs_f64();
    
    let is_first_sample = *prev_rx_bytes == 0 && *prev_tx_bytes == 0;
    
    if !is_first_sample {
        *ema_rx = if *ema_rx == 0.0 { rx_bps } else { STATS_EMA_ALPHA * rx_bps + (1.0 - STATS_EMA_ALPHA) * *ema_rx };
        *ema_tx = if *ema_tx == 0.0 { tx_bps } else { STATS_EMA_ALPHA * tx_bps + (1.0 - STATS_EMA_ALPHA) * *ema_tx };
        
        throughput.push(*ema_tx as usize);
        rx_throughput.push(*ema_rx as usize);
        tx_throughput.push(*ema_tx as usize);
    }
    
    *prev_rx_bytes = stats.rx_bytes;
    *prev_tx_bytes = stats.tx_bytes;
    *last_stats_time = now;
}
```

This is clean. I'll use this in the plan.

- [ ] **Step 3: Add idle decay on `GlobalTick`**

In the `Message::GlobalTick` arm, after spinner tick:

```rust
Message::GlobalTick => {
    if let Some(spinner) = self.loading.as_mut() {
        spinner.tick();
    }
    // Idle decay
    let now = Instant::now();
    for item in &self.connected.list {
        let mut data = item.borrow_mut();
        if let ConnectionData::WifiActive {
            ref mut last_stats_time,
            ref mut ema_rx,
            ref mut ema_tx,
            ref mut throughput,
            ref mut rx_throughput,
            ref mut tx_throughput,
            ..
        } | ConnectionData::WiredActive {
            ref mut last_stats_time,
            ref mut ema_rx,
            ref mut ema_tx,
            ref mut throughput,
            ref mut rx_throughput,
            ref mut tx_throughput,
            ..
        } = &mut *data {
            if now.duration_since(*last_stats_time) > IDLE_DECAY_THRESHOLD {
                *ema_rx = 0.0;
                *ema_tx = 0.0;
                throughput.push(0);
                rx_throughput.push(0);
                tx_throughput.push(0);
                *last_stats_time = now;
            }
        }
    }
}
```

Wait, Rust doesn't support `|` in `let` patterns like that. Need separate matches or a macro. Let me just duplicate:

```rust
Message::GlobalTick => {
    if let Some(spinner) = self.loading.as_mut() {
        spinner.tick();
    }
    let now = Instant::now();
    for item in &self.connected.list {
        let mut data = item.borrow_mut();
        match &mut *data {
            ConnectionData::WifiActive {
                ref mut last_stats_time,
                ref mut ema_rx,
                ref mut ema_tx,
                ref mut throughput,
                ref mut rx_throughput,
                ref mut tx_throughput,
                ..
            } | ConnectionData::WiredActive {
                ref mut last_stats_time,
                ref mut ema_rx,
                ref mut ema_tx,
                ref mut throughput,
                ref mut rx_throughput,
                ref mut tx_throughput,
                ..
            } => {
                if now.duration_since(*last_stats_time) > IDLE_DECAY_THRESHOLD {
                    *ema_rx = 0.0;
                    *ema_tx = 0.0;
                    throughput.push(0);
                    rx_throughput.push(0);
                    tx_throughput.push(0);
                    *last_stats_time = now;
                }
            }
            _ => {}
        }
    }
}
```

Actually, Rust doesn't support `|` in struct patterns with different variant names. This won't compile. We need a helper or match on each variant separately. For the plan, I'll write a helper `apply_idle_decay`.

```rust
fn apply_idle_decay(data: &mut ConnectionData, now: Instant) {
    let (last_stats_time, ema_rx, ema_tx, throughput, rx_throughput, tx_throughput) = match data {
        ConnectionData::WifiActive {
            last_stats_time, ema_rx, ema_tx, throughput, rx_throughput, tx_throughput, ..
        } => (last_stats_time, ema_rx, ema_tx, throughput, rx_throughput, tx_throughput),
        ConnectionData::WiredActive {
            last_stats_time, ema_rx, ema_tx, throughput, rx_throughput, tx_throughput, ..
        } => (last_stats_time, ema_rx, ema_tx, throughput, rx_throughput, tx_throughput),
        ConnectionData::WifiInactive { .. } => return,
    };
    
    if now.duration_since(*last_stats_time) > IDLE_DECAY_THRESHOLD {
        *ema_rx = 0.0;
        *ema_tx = 0.0;
        throughput.push(0);
        rx_throughput.push(0);
        tx_throughput.push(0);
        *last_stats_time = now;
    }
}
```

- [ ] **Step 4: Initialize new fields in `map_active_to_connection_data`**

In `home.rs`, update the `ConnectionData::WifiActive` and `ConnectionData::WiredActive` constructors to include:

```rust
last_stats_time: Instant::now(),
prev_rx_bytes: 0,
prev_tx_bytes: 0,
ema_rx: 0.0,
ema_tx: 0.0,
```

Also update the fallback `_ => ConnectionData::WiredActive` arm.

- [ ] **Step 5: Verify build**

Run: `cargo check --features mock`
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add src/application/home.rs
git commit -m "feat: add EMA smoothing and idle decay for throughput stats"
```

---

## Task 6: Update MockBackend for Incremental Counters

**Files:**
- Modify: `src/backend/mock.rs`

- [ ] **Step 1: Change `device_stats` to use a counter seed**

In `MockState`, change `device_stats: HashMap<String, DeviceStats>` to store mutable counter state. Actually, the simplest change: in `get_device_stats`, instead of adding a fixed amount, add a randomized amount that varies per call.

Current code:
```rust
stats.rx_bytes += 1_000;
stats.tx_bytes += 500;
```

Change to:
```rust
// Add random increments to simulate realistic variable traffic
let rx_inc = 500 + (rand::random::<u64>() % 2_000);
let tx_inc = 200 + (rand::random::<u64>() % 1_000);
stats.rx_bytes += rx_inc;
stats.tx_bytes += tx_inc;
stats.rx_packets += (rx_inc / 100).max(1);
stats.tx_packets += (tx_inc / 100).max(1);
```

This makes sparklines visibly different each tick.

- [ ] **Step 2: Verify build**

Run: `cargo check --features mock`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/backend/mock.rs
git commit -m "feat: make mock device stats return incremental counters"
```

---

## Task 7: Integration Test — Verify Stats Flow End-to-End

**Files:**
- Modify: `src/tests/mod.rs` (or add `src/tests/throughput.rs`)

- [ ] **Step 1: Add a throughput test**

Create `src/tests/throughput.rs`:

```rust
use std::rc::Rc;
use std::cell::RefCell;
use std::time::{Duration, Instant};

use circular_buffer::CircularBuffer;

use crate::application::home::connected::ConnectionData;
use crate::backend::types::DeviceStats;

#[test]
fn test_stats_delta_computation() {
    let mut data = ConnectionData::WiredActive {
        interface: "eth0".into(),
        ip: "192.168.1.42/24".into(),
        name: "Test".into(),
        versions: "v4".into(),
        throughput: CircularBuffer::new(),
        gateway: "192.168.1.1".into(),
        dns: vec![],
        mac: "00:00:00:00:00:00".into(),
        mtu: 1500,
        link_speed: "1 Gbps".into(),
        autoconnect: true,
        uptime: Duration::from_secs(60),
        rx_bytes: 0,
        tx_bytes: 0,
        rx_throughput: CircularBuffer::new(),
        tx_throughput: CircularBuffer::new(),
        last_stats_time: Instant::now() - Duration::from_millis(500),
        prev_rx_bytes: 0,
        prev_tx_bytes: 0,
        ema_rx: 0.0,
        ema_tx: 0.0,
    };
    
    // Simulate first sample (should be skipped — just stores counters)
    // ... we'd need the update_stats helper, but it's private in home.rs
}
```

Hmm, this test needs access to `update_stats` which is private in `home.rs`. Options:
1. Make `update_stats` and `apply_idle_decay` `pub(crate)`
2. Test via `HomeData::update` directly with `Message::BridgeResult`
3. Skip the fine-grained test and just verify with the mock app

For simplicity, I'll make the helpers `pub(crate)` and test them directly.

Actually, even simpler: test that the mock backend increments. The visual verification is running the app. For unit tests, let's test a pure helper:

```rust
// In home.rs, make pub(crate):
pub(crate) fn update_stats(
    last_stats_time: &mut Instant,
    prev_rx_bytes: &mut u64,
    prev_tx_bytes: &mut u64,
    ema_rx: &mut f64,
    ema_tx: &mut f64,
    throughput: &mut CircularBuffer<48, usize>,
    rx_throughput: &mut CircularBuffer<48, usize>,
    tx_throughput: &mut CircularBuffer<48, usize>,
    stats: &DeviceStats,
) { ... }
```

Then test it.

I'll add this to the plan but keep it brief. The point is: make the helpers testable, write a test.

```rust
#[test]
fn test_first_sample_skipped() {
    use crate::application::home::{update_stats, apply_idle_decay};
    
    let mut last_stats_time = Instant::now() - Duration::from_millis(500);
    let mut prev_rx = 0u64;
    let mut prev_tx = 0u64;
    let mut ema_rx = 0.0;
    let mut ema_tx = 0.0;
    let mut throughput = CircularBuffer::new();
    let mut rx_tp = CircularBuffer::new();
    let mut tx_tp = CircularBuffer::new();
    
    let stats = DeviceStats {
        device: "eth0".into(),
        rx_bytes: 10_000,
        tx_bytes: 5_000,
        rx_packets: 100,
        tx_packets: 50,
        signal_dbm: 0,
        tx_rate: 0,
        rx_rate: 0,
    };
    
    update_stats(
        &mut last_stats_time, &mut prev_rx, &mut prev_tx,
        &mut ema_rx, &mut ema_tx,
        &mut throughput, &mut rx_tp, &mut tx_tp,
        &stats,
    );
    
    assert!(throughput.is_empty(), "First sample should not push to buffer");
    assert_eq!(prev_rx, 10_000);
    assert_eq!(prev_tx, 5_000);
}

#[test]
fn test_second_sample_computes_ema() {
    use crate::application::home::update_stats;
    
    let mut last_stats_time = Instant::now() - Duration::from_millis(500);
    let mut prev_rx = 10_000u64;
    let mut prev_tx = 5_000u64;
    let mut ema_rx = 0.0;
    let mut ema_tx = 0.0;
    let mut throughput = CircularBuffer::new();
    let mut rx_tp = CircularBuffer::new();
    let mut tx_tp = CircularBuffer::new();
    
    let stats = DeviceStats {
        device: "eth0".into(),
        rx_bytes: 20_000,
        tx_bytes: 10_000,
        rx_packets: 200,
        tx_packets: 100,
        signal_dbm: 0,
        tx_rate: 0,
        rx_rate: 0,
    };
    
    update_stats(
        &mut last_stats_time, &mut prev_rx, &mut prev_tx,
        &mut ema_rx, &mut ema_tx,
        &mut throughput, &mut rx_tp, &mut tx_tp,
        &stats,
    );
    
    assert_eq!(throughput.len(), 1);
    assert!(ema_tx > 0.0);
}

#[test]
fn test_idle_decay_zeros_ema() {
    use crate::application::home::apply_idle_decay;
    use crate::application::home::connected::ConnectionData;
    
    let mut data = ConnectionData::WiredActive {
        interface: "eth0".into(),
        ip: "192.168.1.42/24".into(),
        name: "Test".into(),
        versions: "v4".into(),
        throughput: CircularBuffer::new(),
        gateway: "192.168.1.1".into(),
        dns: vec![],
        mac: "00:00:00:00:00:00".into(),
        mtu: 1500,
        link_speed: "1 Gbps".into(),
        autoconnect: true,
        uptime: Duration::from_secs(60),
        rx_bytes: 0,
        tx_bytes: 0,
        rx_throughput: CircularBuffer::new(),
        tx_throughput: CircularBuffer::new(),
        last_stats_time: Instant::now() - Duration::from_secs(5), // way past threshold
        prev_rx_bytes: 100_000,
        prev_tx_bytes: 50_000,
        ema_rx: 1_000_000.0,
        ema_tx: 500_000.0,
    };
    
    apply_idle_decay(&mut data, Instant::now());
    
    if let ConnectionData::WiredActive { ema_rx, ema_tx, throughput, .. } = &data {
        assert_eq!(*ema_rx, 0.0);
        assert_eq!(*ema_tx, 0.0);
        assert_eq!(throughput.len(), 1);
        assert_eq!(throughput.iter().next().copied().unwrap(), 0);
    } else {
        panic!("Expected WiredActive");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --features mock throughput
```
Expected: All 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/tests/throughput.rs src/application/home.rs src/tests/mod.rs
git commit -m "test: add throughput delta, EMA, and idle decay tests"
```

---

## Task 8: Final Verification

- [ ] **Step 1: Full test suite**

```bash
cargo test --features mock
```
Expected: All tests pass.

- [ ] **Step 2: Run the app visually**

```bash
cargo run --features mock
```
Wait for loading to finish (the active connections will appear). Verify:
- The Tx sparkline next to each active connection updates visibly (should show progression since mock increments)
- Press `i` on an active connection → info overlay shows Rx and Tx sparklines
- Press `esc` or `q` → back to home list
- Press `q` → quit

- [ ] **Step 3: Commit any fixes**

If any build or runtime issues found, fix and commit.

- [ ] **Step 4: Final commit**

```bash
git commit -m "feat: wire throughput stats collection and display"
```

---

## Spec Self-Review

**1. Spec coverage:**
- ✅ Batch `PollStats` command → Task 1
- ✅ `Rc<RefCell<ConnectionData>>` → Task 3
- ✅ EMA smoothing (α=2/6) → Task 5
- ✅ First-sample skip → Task 5 (via `is_first_sample` check)
- ✅ Idle decay (1s) → Task 5
- ✅ Application `GlobalTick` gating → Task 4
- ✅ Mock incremental counters → Task 6
- ✅ Info overlay Rx+Tx sparklines → already works, just needs `Rc<RefCell>` wiring in Task 3

**2. Placeholder scan:**
- No "TBD" or "TODO" in plan steps.
- All code is concrete.
- No "appropriate" or "handle edge cases" without specifics.

**3. Type consistency:**
- `PollStats` uses `Vec<String>` for interfaces throughout.
- `Rc<RefCell<ConnectionData>>` is the type in `Connected`, `ConnectedList`, `ConnectionItem`, `InfoTarget::Connection`.
- `DeviceStats` result is `BridgeResult::DeviceStats(BridgeResultType<DeviceStats>)` — matches existing.
- EMA fields are `f64`, buffer push is `as usize`.

**4. Gap check:**
- The info overlay's `render_throughput_sparklines` already reads `rx_throughput` and `tx_throughput` from `ConnectionData`. No plan task needed — the wiring in Task 3 makes it work.
- Home list's single sparkline already reads `throughput`. Again, Task 5 pushes into it.
- Connection creation in `rebuild_connected_list` is covered by Task 3.

No gaps found.

---

*Plan written 2026-05-21. Ready for execution.*