//! Synchronization primitives for the kernel.
//!
//! Because this is a bare-metal kernel without an underlying operating system,
//! standard mutexes (which rely on OS thread-parking and system calls) are not available.
//! A Spinlock is the foundational synchronization mechanism in OS kernels:
//! tasks repeatedly check (spin) on an atomic boolean until the lock becomes free.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// A mutual exclusion primitive based on busy-waiting (spinning).
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

    /// Acquires the lock, spinning until it is available.
    /// Returns an RAII guard that automatically releases the lock on drop.
    pub fn lock(&self) -> SpinlockGuard<'_, T> {
        // Test-and-Test-and-Set (TTAS) pattern:
        // Try to atomically swap `true` into `locked`.
        // If it was already `true`, spin using cheap relaxed reads to minimize bus contention.
        while self.locked.swap(true, Ordering::Acquire) {
            while self.locked.load(Ordering::Relaxed) {
                // Emits the x86 `pause` instruction, reducing CPU pipeline strain while spinning.
                core::hint::spin_loop();
            }
        }
        SpinlockGuard { lock: self }
    }
}

/// An RAII implementation of a "scoped lock" of a spinlock.
/// When this structure is dropped (falls out of scope), the lock is automatically released.
pub struct SpinlockGuard<'a, T> {
    lock: &'a Spinlock<T>,
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
    }
}
