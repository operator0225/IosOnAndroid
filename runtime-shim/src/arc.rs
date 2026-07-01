//! Automatic Reference Counting's runtime half: `retain`/`release`
//! bookkeeping. (The *compile-time* half — a compiler inserting the right
//! retain/release calls — is out of scope here; this is what those calls
//! bottom out in.)
//!
//! Real ARC on Darwin stores the refcount inline in the object header (or
//! in a side table for tagged pointers) and an over-release is undefined
//! behavior — typically a crash, sometime later, somewhere else, which is
//! exactly the kind of bug this project would rather not blindly
//! reproduce. [`RefCounted::release`] instead reports
//! [`ReleaseOutcome::AlreadyZero`] so a caller can treat it as the logic
//! error it is.

use std::sync::atomic::{AtomicUsize, Ordering};

pub struct RefCounted {
    count: AtomicUsize,
}

impl Default for RefCounted {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseOutcome {
    /// The object is still referenced; carries the new count.
    StillAlive(usize),
    /// This was the release that brought the count to zero — the caller
    /// should run `dealloc` now.
    ShouldDeallocate,
    /// The count was already zero: an over-release bug in the caller.
    AlreadyZero,
}

impl RefCounted {
    /// A freshly allocated object starts with one implicit owning
    /// reference, matching `+alloc`/`+new`'s convention.
    pub fn new() -> Self {
        RefCounted {
            count: AtomicUsize::new(1),
        }
    }

    pub fn retain(&self) -> usize {
        self.count.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn release(&self) -> ReleaseOutcome {
        loop {
            let current = self.count.load(Ordering::Acquire);
            if current == 0 {
                return ReleaseOutcome::AlreadyZero;
            }
            let new = current - 1;
            if self
                .count
                .compare_exchange_weak(current, new, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return if new == 0 {
                    ReleaseOutcome::ShouldDeallocate
                } else {
                    ReleaseOutcome::StillAlive(new)
                };
            }
        }
    }

    pub fn retain_count(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn starts_with_one_owning_reference() {
        assert_eq!(RefCounted::new().retain_count(), 1);
    }

    #[test]
    fn retain_increments_and_release_decrements() {
        let rc = RefCounted::new();
        assert_eq!(rc.retain(), 2);
        assert_eq!(rc.retain(), 3);
        assert_eq!(rc.release(), ReleaseOutcome::StillAlive(2));
        assert_eq!(rc.release(), ReleaseOutcome::StillAlive(1));
    }

    #[test]
    fn final_release_signals_deallocate_exactly_once() {
        let rc = RefCounted::new();
        assert_eq!(rc.release(), ReleaseOutcome::ShouldDeallocate);
        assert_eq!(rc.retain_count(), 0);
    }

    #[test]
    fn over_release_is_reported_not_ub() {
        let rc = RefCounted::new();
        assert_eq!(rc.release(), ReleaseOutcome::ShouldDeallocate);
        // A second release past zero must not panic or wrap around to
        // usize::MAX; it must be reported.
        assert_eq!(rc.release(), ReleaseOutcome::AlreadyZero);
        assert_eq!(rc.retain_count(), 0);
    }

    #[test]
    fn concurrent_retain_release_never_underflows_the_base_reference() {
        let rc = Arc::new(RefCounted::new());
        const THREADS: usize = 8;
        const ITERS: usize = 10_000;

        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let rc = Arc::clone(&rc);
                thread::spawn(move || {
                    for _ in 0..ITERS {
                        rc.retain();
                        let outcome = rc.release();
                        // The base reference from `new()` is never
                        // released by these workers, so the count can
                        // never legitimately hit zero here.
                        assert_ne!(outcome, ReleaseOutcome::AlreadyZero);
                        assert_ne!(outcome, ReleaseOutcome::ShouldDeallocate);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(rc.retain_count(), 1);
    }
}
