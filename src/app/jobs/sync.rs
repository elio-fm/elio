use std::sync::{Condvar, Mutex, MutexGuard};

pub(super) fn lock_unpoison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

pub(super) fn wait_unpoison<'a, T>(
    condvar: &Condvar,
    guard: MutexGuard<'a, T>,
) -> MutexGuard<'a, T> {
    condvar
        .wait(guard)
        .unwrap_or_else(|poison| poison.into_inner())
}
