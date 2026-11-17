//! Synchronization primitives for the kernel.
//!
//! Because this is a bare-metal kernel without an underlying operating system,
//! standard mutexes (which rely on OS thread-parking and system calls) are not available.
//! A Spinlock is the foundational synchronization mechanism in OS kernels:
//! tasks repeatedly check (spin) on an atomic boolean until the lock becomes free.
//!
//! To prevent single-core deadlocks with interrupt handlers, this implementation
//! is interrupt-safe: it disables interrupts before taking the lock and restores
//! the previous interrupt state when the lock guard is dropped.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// An interrupt-safe mutual exclusion primitive based on busy-waiting (spinning).
pub struct Spinlock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

// Safety: We can safely share Spinlock between threads if T is Send.
unsafe impl<T: Send> Sync for Spinlock<T> {}
unsafe impl<T: Send> Send for Spinlock<T> {}

impl<T> Spinlock<T> {
    /// Creates a new, unlocked Spinlock wrapping the given data.
    /// This is a `const fn` to allow static initialization at compile time.
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquires the lock while disabling interrupts on the local CPU.
    ///
    /// Preserves the previous interrupt state and restores it on guard drop.
    /// This prevents deadlocks where an interrupt handler interrupts a critical section
    /// and attempts to acquire the same lock on the same CPU core.
    pub fn lock(&self) -> SpinlockGuard<'_, T> {
        // Query whether interrupts are currently enabled before disabling them.
        let interrupts_were_enabled = x86_64::instructions::interrupts::are_enabled();
        x86_64::instructions::interrupts::disable();

        // Test-and-Test-and-Set (TTAS) pattern:
        while self.locked.swap(true, Ordering::Acquire) {
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }

        SpinlockGuard {
            lock: self,
            interrupts_were_enabled,
        }
    }
}

/// An RAII implementation of a "scoped lock" of a spinlock.
/// When dropped, releases the lock and restores the previous interrupt state.
pub struct SpinlockGuard<'a, T> {
    lock: &'a Spinlock<T>,
    interrupts_were_enabled: bool,
}

impl<'a, T> Deref for SpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // Safety: We hold the spinlock, so shared read access is guaranteed to be safe.
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for SpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        // Safety: We hold the spinlock, so exclusive mutable access is guaranteed.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for SpinlockGuard<'a, T> {
    fn drop(&mut self) {
        // Ordering::Release ensures all previous operations finish before releasing the lock.
        self.lock.locked.store(false, Ordering::Release);

        // Only restore interrupt flag if interrupts were previously enabled.
        // Never blindly enable interrupts if they were disabled before acquiring the lock!
        if self.interrupts_were_enabled {
            x86_64::instructions::interrupts::enable();
        }
    }
}
