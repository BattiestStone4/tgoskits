//! Time management module.

use alloc::{
    borrow::ToOwned,
    collections::binary_heap::BinaryHeap,
    sync::{Arc, Weak},
};
use core::{mem, time::Duration};

use ax_lazyinit::LazyLock;
use ax_runtime::hal::time::{NANOS_PER_SEC, TimeValue, monotonic_time_nanos, wall_time};
use ax_task::{
    WeakAxTaskRef, current,
    future::{block_on, timeout_at_wall},
};
use event_listener::{Event, listener};
use starry_signal::Signo;
use strum::FromRepr;

use crate::{
    sync::IrqMutex as Mutex,
    task::{PidIdentity, poll_process_timer, poll_timer},
};

fn time_value_from_nanos(nanos: usize) -> TimeValue {
    let secs = nanos as u64 / NANOS_PER_SEC;
    let nsecs = nanos as u64 - secs * NANOS_PER_SEC;
    TimeValue::new(secs, nsecs as u32)
}

#[derive(Debug, Clone)]
pub enum AlarmTarget {
    Thread(WeakAxTaskRef),
    Process(Weak<PidIdentity>),
}

struct Entry {
    deadline: Duration,
    target: AlarmTarget,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        other.deadline.cmp(&self.deadline)
    }
}

static ALARM_LIST: LazyLock<Mutex<BinaryHeap<Entry>>> =
    LazyLock::new(|| Mutex::new(BinaryHeap::new()));

static EVENT_NEW_TIMER: LazyLock<Event> = LazyLock::new(Event::new);

/// The type of interval timer.
#[repr(i32)]
#[allow(non_camel_case_types)]
#[derive(Eq, PartialEq, Debug, Clone, Copy, FromRepr)]
pub enum ITimerType {
    /// 统计系统实际运行时间
    Real    = 0,
    /// 统计用户态运行时间
    Virtual = 1,
    /// 统计进程的所有用户态/内核态运行时间
    Prof    = 2,
}

impl ITimerType {
    /// Returns the signal number associated with this timer type.
    pub fn signo(&self) -> Signo {
        match self {
            ITimerType::Real => Signo::SIGALRM,
            ITimerType::Virtual => Signo::SIGVTALRM,
            ITimerType::Prof => Signo::SIGPROF,
        }
    }
}

#[derive(Default)]
struct ITimer {
    interval_ns: usize,
    remained_ns: usize,
}

impl ITimer {
    pub fn new(interval_ns: usize, remained_ns: usize) -> Self {
        let result = Self {
            interval_ns,
            remained_ns,
        };
        result.renew_timer();
        result
    }

    pub fn update(&mut self, delta: usize) -> bool {
        if self.remained_ns == 0 {
            return false;
        }
        if self.remained_ns > delta {
            self.remained_ns -= delta;
            false
        } else {
            self.remained_ns = self.interval_ns;
            self.renew_timer();
            true
        }
    }

    pub fn renew_timer(&self) {
        if self.remained_ns > 0 {
            let deadline = wall_time() + Duration::from_nanos(self.remained_ns as u64);
            register_alarm(deadline);
        }
    }
}

/// The process-wide `ITIMER_REAL` state shared by every thread.
#[derive(Default)]
pub(crate) struct ProcessRealTimer {
    interval: TimeValue,
    deadline: Option<TimeValue>,
}

impl ProcessRealTimer {
    /// Replaces the timer and returns its previous interval and remaining time.
    pub fn set(
        &mut self,
        identity: &Arc<PidIdentity>,
        interval_ns: usize,
        remaining_ns: usize,
    ) -> (TimeValue, TimeValue) {
        let old = self.get();
        self.interval = TimeValue::from_nanos(interval_ns as u64);
        self.deadline = (remaining_ns != 0).then(|| {
            let deadline = wall_time() + TimeValue::from_nanos(remaining_ns as u64);
            register_alarm_for(deadline, AlarmTarget::Process(Arc::downgrade(identity)));
            deadline
        });
        old
    }

    /// Returns the timer interval and the time remaining before expiration.
    pub fn get(&self) -> (TimeValue, TimeValue) {
        let remaining = self
            .deadline
            .map(|deadline| deadline.saturating_sub(wall_time()))
            .unwrap_or_default();
        (self.interval, remaining)
    }

    /// Advances an expired timer and reports whether `SIGALRM` must be emitted.
    pub fn poll_expired(&mut self, identity: &Arc<PidIdentity>) -> bool {
        let Some(deadline) = self.deadline else {
            return false;
        };
        if wall_time() < deadline {
            return false;
        }

        if self.interval.is_zero() {
            self.deadline = None;
        } else {
            let deadline = wall_time() + self.interval;
            self.deadline = Some(deadline);
            register_alarm_for(deadline, AlarmTarget::Process(Arc::downgrade(identity)));
        }
        true
    }
}

/// Register an alarm at the given wall-clock deadline for the current task.
/// Used by both ITimer and POSIX timers.
pub fn register_alarm(deadline: Duration) {
    register_alarm_for(deadline, AlarmTarget::Thread(Arc::downgrade(&current())));
}

/// Register an alarm at the given wall-clock deadline for a specific target.
/// Used when re-arming periodic POSIX timers from the alarm_task context,
/// where `current()` is the alarm_task, not the user task.
pub fn register_alarm_for(deadline: Duration, target: AlarmTarget) {
    let mut guard = ALARM_LIST.lock();
    let should_wake = guard.peek().is_none_or(|it| it.deadline > deadline);
    guard.push(Entry { deadline, target });
    drop(guard);
    if should_wake {
        EVENT_NEW_TIMER.notify(1);
    }
}

/// Represents the state of the timer.
#[derive(Debug)]
pub enum TimerState {
    /// Fallback state.
    None,
    /// The timer is running in user space.
    User,
    /// The timer is running in kernel space.
    Kernel,
}

/// A manager for time-related operations.
pub struct TimeManager {
    utime_ns: usize,
    stime_ns: usize,
    /// Baseline for itimer delta calculation in `poll()`.
    /// Updated only by `poll()`, never by `tick()`.
    last_wall_ns: usize,
    /// Baseline for tick-based CPU time accumulation.
    /// Updated by `tick()` and synced to `last_wall_ns` at the end of `poll()`.
    last_tick_ns: usize,
    state: TimerState,
    itimers: [ITimer; 3],
}

impl Default for TimeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeManager {
    pub(crate) fn new() -> Self {
        Self {
            utime_ns: 0,
            stime_ns: 0,
            last_wall_ns: 0,
            last_tick_ns: 0,
            state: TimerState::None,
            itimers: Default::default(),
        }
    }

    /// Returns the current user time and system time as a tuple of `TimeValue`.
    pub fn output(&self) -> (TimeValue, TimeValue) {
        let utime = time_value_from_nanos(self.utime_ns);
        let stime = time_value_from_nanos(self.stime_ns);
        (utime, stime)
    }

    /// Accumulates CPU time for the current tick without emitting signals.
    ///
    /// Safe to call from IRQ/timer-callback context.  Signal-bearing itimers
    /// are checked only through the full `poll()` path at syscall boundaries.
    ///
    /// Uses `last_tick_ns` as the exclusive baseline so that `poll()`'s
    /// itimer accounting (which uses the independent `last_wall_ns`) is not
    /// affected.
    pub fn tick(&mut self) {
        let now_ns = monotonic_time_nanos() as usize;
        let delta = now_ns.saturating_sub(self.last_tick_ns);
        match self.state {
            TimerState::User => self.utime_ns += delta,
            TimerState::Kernel => self.stime_ns += delta,
            TimerState::None => {}
        }
        self.last_tick_ns = now_ns;
        // last_wall_ns is intentionally NOT touched here so that poll()
        // continues to see the full wall-clock delta for itimer accounting.
    }

    /// Polls the time manager to update CPU time and interval timers,
    /// returning the interval-timer signals that fired (at most 2, in slot
    /// order Virtual/Prof; `ITIMER_REAL` is process state and is polled
    /// separately by `poll_process_timer`).
    ///
    /// The caller MUST emit the returned signals AFTER releasing the `time`
    /// lock: signal delivery takes other locks, and the `time` lock is
    /// IRQ-disabling — running the emitter under it would extend the IRQs-off
    /// window and risk a lock-ordering deadlock. Returning the signals keeps
    /// the locked region free of any nested lock.
    #[must_use = "the returned itimer signals must be emitted after unlocking"]
    pub fn poll(&mut self) -> [Option<Signo>; 3] {
        let now_ns = monotonic_time_nanos() as usize;
        // itimer_delta: full wall-clock time since the last poll() call.
        // Used for interval-timer accounting so they fire at the right time
        // regardless of whether tick() has been called in between.
        let itimer_delta = now_ns.saturating_sub(self.last_wall_ns);
        // remaining: time since the last tick() that has not yet been counted
        // in utime_ns / stime_ns.  If tick() was never called, last_tick_ns ==
        // last_wall_ns and remaining == itimer_delta (identical to original).
        let remaining = now_ns.saturating_sub(self.last_tick_ns);
        // Fixed slots so no `n` counter is needed: 0=Virtual, 1=Prof.
        let mut fired = [None; 3];
        match self.state {
            TimerState::User => {
                self.utime_ns += remaining;
                if self.itimers[ITimerType::Virtual as usize].update(itimer_delta) {
                    fired[0] = Some(ITimerType::Virtual.signo());
                }
                if self.itimers[ITimerType::Prof as usize].update(itimer_delta) {
                    fired[1] = Some(ITimerType::Prof.signo());
                }
            }
            TimerState::Kernel => {
                self.stime_ns += remaining;
                if self.itimers[ITimerType::Prof as usize].update(itimer_delta) {
                    fired[1] = Some(ITimerType::Prof.signo());
                }
            }
            TimerState::None => {}
        }
        // `ITIMER_REAL` is process state and is polled separately, so the
        // Real slot (2) is never filled here.
        self.last_wall_ns = now_ns;
        // Sync tick baseline with poll baseline so the next tick() starts
        // from a clean slate.
        self.last_tick_ns = now_ns;
        fired
    }

    /// Updates the timer state.
    pub fn set_state(&mut self, state: TimerState) {
        self.state = state;
    }

    /// Sets the interval timer of the specified type with the given interval
    /// and remaining time.
    pub fn set_itimer(
        &mut self,
        ty: ITimerType,
        interval_ns: usize,
        remained_ns: usize,
    ) -> (TimeValue, TimeValue) {
        let old = mem::replace(
            &mut self.itimers[ty as usize],
            ITimer::new(interval_ns, remained_ns),
        );
        (
            time_value_from_nanos(old.interval_ns),
            time_value_from_nanos(old.remained_ns),
        )
    }

    /// Gets the current interval and remaining time.
    pub fn get_itimer(&self, ty: ITimerType) -> (TimeValue, TimeValue) {
        let itimer = &self.itimers[ty as usize];
        (
            time_value_from_nanos(itimer.interval_ns),
            time_value_from_nanos(itimer.remained_ns),
        )
    }
}

async fn alarm_task() {
    loop {
        let mut guard = ALARM_LIST.lock();
        let Some(entry) = guard.peek() else {
            drop(guard);
            listener!(EVENT_NEW_TIMER => listener);

            if !ALARM_LIST.lock().is_empty() {
                continue;
            }
            listener.await;

            continue;
        };

        let now = wall_time();
        if entry.deadline <= now {
            let entry_deadline = entry.deadline;
            let target = entry.target.clone();
            assert!(guard.pop().is_some_and(|it| it.deadline == entry_deadline));
            drop(guard);
            match target {
                AlarmTarget::Thread(weak_task) => {
                    if let Some(task) = weak_task.upgrade() {
                        poll_timer(&task);
                    }
                }
                AlarmTarget::Process(identity) => {
                    if let Some(identity) = identity.upgrade() {
                        poll_process_timer(&identity);
                    }
                }
            }
        } else {
            let deadline = entry.deadline;
            drop(guard);
            listener!(EVENT_NEW_TIMER => listener);
            if ALARM_LIST
                .lock()
                .peek()
                .is_none_or(|it| it.deadline != deadline)
            {
                continue;
            }
            let _ = timeout_at_wall(Some(deadline), listener).await;
        }
    }
}

/// Spawns the alarm task.
pub fn spawn_alarm_task() {
    info!("Initialize alarm...");
    ax_task::spawn_raw(
        || block_on(alarm_task()),
        "alarm_task".to_owned(),
        ax_task::default_task_stack_size(),
    );
}

#[cfg(all(test, not(axtest)))]
fn itimer_type_signo_and_time_conversion_rules_hold_for_test() -> bool {
    // ITimerType::signo returns a Signo for each variant without panicking.
    let _real = ITimerType::Real.signo();
    let _virt = ITimerType::Virtual.signo();
    let _prof = ITimerType::Prof.signo();

    // time_value_from_nanos: converts nanoseconds to TimeValue without panicking.
    let _ = time_value_from_nanos(0);
    let _ = time_value_from_nanos(1);
    let _ = time_value_from_nanos(1000000000usize);

    true
}

/// The `ITimer::update` accounting contract that `TimeManager::poll()` relies
/// on to decide the fired set: `remained == 0` never fires, a not-yet-elapsed
/// timer decrements without firing, an elapsed one-shot fires and disarms
/// (`remained` becomes 0), and an elapsed periodic one re-arms from its
/// `interval`. Pure logic, so it is deterministic under the frozen host
/// clock (`DummyTime::current_ticks` returns 0 under host tests).
#[cfg(all(test, not(axtest)))]
fn itimer_update_rules_for_test() -> bool {
    use crate::task::timer::ITimer;

    // remained == 0: never fires, unchanged.
    let mut t = ITimer {
        interval_ns: 0,
        remained_ns: 0,
    };
    if t.update(usize::MAX) || t.remained_ns != 0 {
        return false;
    }

    // Not yet elapsed: decrements, no fire.
    let mut t = ITimer {
        interval_ns: 0,
        remained_ns: 10,
    };
    if t.update(4) || t.remained_ns != 6 {
        return false;
    }

    // Elapsed one-shot: fires, disarms (interval=0 keeps remained at 0, so
    // the fire path's `renew_timer` has no alarm to register and stays host
    // testable).
    let mut t = ITimer {
        interval_ns: 0,
        remained_ns: 1,
    };
    if !t.update(1) || t.remained_ns != 0 {
        return false;
    }
    // Disarmed timers stay silent forever.
    if t.update(usize::MAX) || t.remained_ns != 0 {
        return false;
    }
    // Elapsed with a large delta: fires and disarms as well (interval=0).
    let mut t = ITimer {
        interval_ns: 0,
        remained_ns: 1,
    };
    if !t.update(usize::MAX / 2) || t.remained_ns != 0 {
        return false;
    }
    // NOTE: the periodic re-arm branch (interval > 0) calls `renew_timer`,
    // which registers an alarm on `current()` — unavailable in the host test
    // harness — so the interval-rewrite half of that branch is not exercised
    // here; it runs on target in the qemu timer-family tests.

    true
}

/// Regression for #2010's `TimeManager::poll()` contract: it returns the
/// interval-timer signals that fired (fixed slots: 0=Virtual/SIGVTALRM,
/// 1=Prof/SIGPROF) instead of emitting them via a closure, so the caller can
/// deliver them after releasing the IRQ-disabling `time` lock. Also pins the
/// merged upstream semantics that `ITIMER_REAL` is process state and is polled
/// separately by `poll_process_timer` — the Real slot (2) must never be filled
/// here. Runs under the host clock, which is frozen at zero (`DummyTime`), so
/// every poll sees `delta == 0` and no timer can fire: what this test asserts
/// is that `poll()` never fabricates signals, keeps CPU time unchanged at zero
/// delta, resyncs its baselines, and leaves the armed timers intact for a real
/// clock to fire. The fire/expiry semantics themselves are covered by
/// [`itimer_update_rules_for_test`].
///
/// Timers are armed by writing the `ITimer` fields directly instead of going
/// through `set_itimer`: that path also registers an alarm (`current()`), which
/// is unavailable in the host unit-test harness.
#[cfg(all(test, not(axtest)))]
fn time_manager_poll_fired_slots_for_test() -> bool {
    use crate::task::timer::{ITimer, ITimerType, TimeManager, TimerState};

    // Arms an itimer slot without the alarm registration in `set_itimer`.
    fn arm(tm: &mut TimeManager, ty: ITimerType, interval_ns: usize, remained_ns: usize) {
        tm.itimers[ty as usize] = ITimer {
            interval_ns,
            remained_ns,
        };
    }

    // (1) Empty table: nothing fires, and the poll baselines are synced.
    {
        let mut tm = TimeManager::new();
        if tm.poll() != [None, None, None] {
            return false;
        }
        // The end of poll() syncs the tick baseline to the poll baseline.
        if tm.last_wall_ns != tm.last_tick_ns {
            return false;
        }
    }

    // (2) Armed Virtual + Prof timers under a zero delta: no fabricated
    // signals, and CPU time stays untouched (User state would normally add the
    // elapsed delta — with delta == 0 nothing is added).
    {
        let mut tm = TimeManager::new();
        tm.set_state(TimerState::User);
        let _ = tm.poll(); // baseline
        arm(&mut tm, ITimerType::Virtual, 0, 1);
        arm(&mut tm, ITimerType::Prof, 0, 1);
        let (u0, s0) = tm.output();
        if tm.poll() != [None, None, None] {
            return false;
        }
        let (u1, s1) = tm.output();
        if u1.as_nanos() != u0.as_nanos() || s1.as_nanos() != s0.as_nanos() {
            return false;
        }
        // The armed timers survive the poll for a real clock to fire later.
        if tm.itimers[ITimerType::Virtual as usize].remained_ns != 1
            || tm.itimers[ITimerType::Prof as usize].remained_ns != 1
        {
            return false;
        }
    }

    // (3) The Real slot is never filled: ITIMER_REAL is process state, polled
    // separately by poll_process_timer, so even an armed Real timer must not
    // produce a signal through TimeManager::poll.
    {
        let mut tm = TimeManager::new();
        tm.set_state(TimerState::User);
        let _ = tm.poll();
        arm(&mut tm, ITimerType::Real, 0, 1);
        if tm.poll() != [None, None, None] {
            return false;
        }
        if tm.itimers[ITimerType::Real as usize].remained_ns != 1 {
            return false;
        }
    }

    // (4) Kernel state never touches the Virtual slot.
    {
        let mut tm = TimeManager::new();
        tm.set_state(TimerState::Kernel);
        let _ = tm.poll();
        arm(&mut tm, ITimerType::Virtual, 0, 1);
        arm(&mut tm, ITimerType::Prof, 0, 1);
        if tm.poll() != [None, None, None] {
            return false;
        }
        if tm.itimers[ITimerType::Virtual as usize].remained_ns != 1 {
            return false;
        }
    }

    true
}

/// #2010: `Thread.time` is `IrqMutex<TimeManager>`, not a fake-Sync `RefCell`.
/// Hammer the same `TimeManager` from several host threads through the two
/// access patterns the kernel uses — blocking `lock()` at syscall boundaries
/// and `try_lock()` from the IRQ tick path — and require every observed
/// utime+stime total to be monotonically non-decreasing. Under a data race the
/// accounting would tear or the borrow bookkeeping would panic; under the
/// mutex the total is never observed to shrink.
#[cfg(all(test, not(axtest)))]
extern crate std; // host tests: real multi-threaded access needs std::thread

#[cfg(all(test, not(axtest)))]
fn time_manager_concurrent_access_under_irq_mutex_for_test() -> bool {
    use alloc::{sync::Arc, vec::Vec};
    use std::thread;

    use crate::{
        sync::IrqMutex,
        task::timer::{ITimer, ITimerType, TimeManager, TimerState},
    };

    const THREADS: usize = 4;
    const ITERS: usize = 500;

    let tm = Arc::new(IrqMutex::new(TimeManager::new()));
    let mut handles = Vec::new();
    for t in 0..THREADS {
        let tm = tm.clone();
        handles.push(thread::spawn(move || {
            let mut local_max: usize = 0;
            for _ in 0..ITERS {
                {
                    let mut guard = tm.lock();
                    guard.set_state(TimerState::User);
                    let _ = guard.poll();
                }
                // Alternate the two non-blocking accessors: the poll path
                // (alarm task style) and the tick path (timer IRQ style).
                if t % 2 == 0 {
                    if let Some(mut guard) = tm.try_lock() {
                        guard.itimers[ITimerType::Virtual as usize] = ITimer {
                            interval_ns: 0,
                            remained_ns: 1,
                        };
                        let _ = guard.poll();
                    }
                } else if let Some(mut guard) = tm.try_lock() {
                    guard.tick();
                }
                let total = {
                    let guard = tm.lock();
                    let (u, s) = guard.output();
                    u.as_micros() as usize + s.as_micros() as usize
                };
                if total < local_max {
                    return false; // a torn/racing read of the accounting
                }
                local_max = total;
            }
            true
        }));
    }
    handles.into_iter().all(|h| h.join().unwrap())
}

#[cfg(all(test, not(axtest)))]
mod tests {
    #[test]
    fn itimer_type_signo_and_time_conversion_rules_hold() {
        assert!(super::itimer_type_signo_and_time_conversion_rules_hold_for_test());
    }

    #[test]
    fn itimer_update_rules() {
        assert!(super::itimer_update_rules_for_test());
    }

    #[test]
    fn time_manager_poll_fired_slots() {
        assert!(super::time_manager_poll_fired_slots_for_test());
    }

    #[test]
    fn time_manager_concurrent_access_under_irq_mutex() {
        assert!(super::time_manager_concurrent_access_under_irq_mutex_for_test());
    }
}
