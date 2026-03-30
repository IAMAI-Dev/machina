// Virtual clock and timer system.
//
// Provides a monotonic virtual clock with a priority-queue based
// timer scheduler. Timers fire in expiry order when the clock
// is stepped forward.
//
// Reference: QEMU qemu-timer.h / timer.c.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockType {
    Realtime,
    Virtual,
    Host,
}

pub struct VirtualClock {
    clock_type: ClockType,
    ns: AtomicI64,
    enabled: AtomicBool,
    timers: Mutex<BinaryHeap<Reverse<TimerEntry>>>,
    next_id: Mutex<u64>,
}

struct TimerEntry {
    expire_time: i64,
    id: u64,
    callback: Box<dyn FnOnce() + Send>,
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.expire_time == other.expire_time && self.id == other.id
    }
}

impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.expire_time
            .cmp(&other.expire_time)
            .then(self.id.cmp(&other.id))
    }
}

impl VirtualClock {
    pub fn new(clock_type: ClockType) -> Self {
        Self {
            clock_type,
            ns: AtomicI64::new(0),
            enabled: AtomicBool::new(true),
            timers: Mutex::new(BinaryHeap::new()),
            next_id: Mutex::new(0),
        }
    }

    /// Return the clock type.
    pub fn clock_type(&self) -> ClockType {
        self.clock_type
    }

    /// Read current time in nanoseconds (atomic).
    pub fn get_ns(&self) -> i64 {
        self.ns.load(Ordering::Acquire)
    }

    /// Set current time in nanoseconds (atomic).
    pub fn set_ns(&self, ns: i64) {
        self.ns.store(ns, Ordering::Release);
    }

    /// Advance clock by `delta_ns` and fire all expired timers
    /// in chronological order.
    pub fn step(&self, delta_ns: i64) {
        let new_ns = self.ns.fetch_add(delta_ns, Ordering::AcqRel) + delta_ns;

        // Collect expired timers under the lock, then fire
        // callbacks outside the lock to avoid deadlocks.
        let mut expired = Vec::new();
        {
            let mut heap = self.timers.lock().unwrap();
            while let Some(Reverse(entry)) = heap.peek() {
                if entry.expire_time <= new_ns {
                    let Reverse(e) = heap.pop().unwrap();
                    expired.push(e);
                } else {
                    break;
                }
            }
        }

        // Callbacks are already sorted by (expire_time, id)
        // since we popped from a min-heap.
        for entry in expired {
            (entry.callback)();
        }
    }

    /// Schedule a one-shot timer. Returns a unique timer ID
    /// that can be used with `remove_timer`.
    pub fn add_timer(
        &self,
        expire_ns: i64,
        callback: impl FnOnce() + Send + 'static,
    ) -> u64 {
        let mut next = self.next_id.lock().unwrap();
        let id = *next;
        *next += 1;
        drop(next);

        let entry = TimerEntry {
            expire_time: expire_ns,
            id,
            callback: Box::new(callback),
        };
        self.timers.lock().unwrap().push(Reverse(entry));
        id
    }

    /// Cancel a pending timer by ID. Returns `true` if the
    /// timer was found and removed.
    pub fn remove_timer(&self, id: u64) -> bool {
        let mut heap = self.timers.lock().unwrap();
        let old_len = heap.len();

        // Drain, filter, rebuild — O(n) but timers are few.
        let entries: Vec<_> = std::mem::take(&mut *heap)
            .into_vec()
            .into_iter()
            .filter(|Reverse(e)| e.id != id)
            .collect();
        let new_len = entries.len();
        *heap = BinaryHeap::from(entries);

        new_len < old_len
    }

    /// Check whether the clock is enabled.
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Enable or disable the clock.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }
}
