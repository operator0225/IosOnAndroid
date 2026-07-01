//! A futex-backed mutex: one piece of the "pthread/futex-backed threading
//! primitives satisfying Darwin's thread API surface" work from
//! `../../docs/ROADMAP.md` Phase 2.
//!
//! Darwin's own low-level lock (`os_unfair_lock`, and `pthread_mutex_t`
//! under the hood) is kernel-assisted in a way specific to XNU. We don't
//! have XNU (see `../../docs/LEGAL.md`), so a Darwin-targeting binary's
//! locking calls need to bottom out in *something* — this is that
//! something: the standard two/three-state futex mutex algorithm (Ulrich
//! Drepper, "Futexes Are Tricky"), implemented directly against Linux's
//! `futex(2)`. Unlike the ptrace guest-interception loop, this has no
//! ARM64-specific bits at all, so it's fully exercised by real concurrent
//! threads in this crate's tests, on any host.

use std::sync::atomic::{AtomicI32, Ordering};

const UNLOCKED: i32 = 0;
const LOCKED_NO_WAITERS: i32 = 1;
const LOCKED_WAITERS: i32 = 2;

pub struct FutexMutex {
    state: AtomicI32,
}

impl Default for FutexMutex {
    fn default() -> Self {
        Self::new()
    }
}

impl FutexMutex {
    pub const fn new() -> Self {
        FutexMutex {
            state: AtomicI32::new(UNLOCKED),
        }
    }

    pub fn try_lock(&self) -> bool {
        self.state
            .compare_exchange(
                UNLOCKED,
                LOCKED_NO_WAITERS,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_ok()
    }

    pub fn lock(&self) {
        if self.try_lock() {
            return;
        }
        loop {
            // Announce that we're about to wait, so the unlocker knows to
            // wake someone up rather than doing a bare store.
            if self.state.swap(LOCKED_WAITERS, Ordering::Acquire) == UNLOCKED {
                return;
            }
            futex_wait(&self.state, LOCKED_WAITERS);
        }
    }

    pub fn unlock(&self) {
        if self.state.swap(UNLOCKED, Ordering::Release) == LOCKED_WAITERS {
            futex_wake_one(&self.state);
        }
    }
}

fn futex_wait(state: &AtomicI32, expected: i32) {
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            state as *const AtomicI32 as *const i32,
            libc::FUTEX_WAIT,
            expected,
            std::ptr::null::<libc::timespec>(),
        );
    }
    // Spurious wakeups (races where `state` no longer equals `expected` by
    // the time the kernel checks) just send us back around the caller's
    // loop; nothing to handle here.
}

fn futex_wake_one(state: &AtomicI32) {
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            state as *const AtomicI32 as *const i32,
            libc::FUTEX_WAKE,
            1,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn uncontended_lock_unlock_round_trips() {
        let m = FutexMutex::new();
        assert!(m.try_lock());
        assert!(!m.try_lock()); // already held
        m.unlock();
        assert!(m.try_lock());
        m.unlock();
    }

    #[test]
    fn serializes_concurrent_increments() {
        // A plain, unsynchronized `counter += 1` from many threads would
        // lose updates to races; correctness here proves the mutex is
        // actually providing mutual exclusion, not just compiling.
        struct Shared {
            mutex: FutexMutex,
            counter: std::cell::UnsafeCell<u64>,
        }
        unsafe impl Sync for Shared {}

        let shared = Arc::new(Shared {
            mutex: FutexMutex::new(),
            counter: std::cell::UnsafeCell::new(0),
        });

        const THREADS: usize = 8;
        const INCREMENTS: usize = 5_000;

        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    for _ in 0..INCREMENTS {
                        shared.mutex.lock();
                        unsafe {
                            let p = shared.counter.get();
                            *p += 1;
                        }
                        shared.mutex.unlock();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let total = unsafe { *shared.counter.get() };
        assert_eq!(total, (THREADS * INCREMENTS) as u64);
    }

    #[test]
    fn contended_lock_eventually_wakes_waiters() {
        // Forces the LOCKED_WAITERS path (unlike the two tests above,
        // which can complete via try_lock/uncontended fast paths).
        let shared = Arc::new(FutexMutex::new());
        shared.lock();

        let waiter_shared = Arc::clone(&shared);
        let waiter = thread::spawn(move || {
            waiter_shared.lock();
            waiter_shared.unlock();
        });

        thread::sleep(std::time::Duration::from_millis(50));
        shared.unlock();

        waiter.join().unwrap();
    }
}
