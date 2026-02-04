#![feature(downcast_unchecked)]
#![feature(min_specialization)]

#[cfg(target_os = "macos")]
mod codesign;
pub mod erased_executor;
mod error;
mod functions;
mod hot_application;
mod hot_program;
mod hot_state;
mod into_result;
mod lib_reloader;
mod message;
mod reloader;
mod winit;

//Re-export
pub use iced;
pub use iced_futures;
pub use iced_graphics;
pub use serde;
pub use serde_derive;

pub use hot_application::application;
pub use hot_ice_macros::{hot_fn, hot_state};
pub use reloader::ReloaderSettings;

pub mod macro_use {
    pub use super::error::{HotIceError, HotResult};
    pub use super::hot_state::{DynState, HotState};
    pub use super::message::{DynMessage, HotMessage};

    use std::cell::Cell;

    thread_local! {
        static PANIC_LOCATION: Cell<Option<(&'static str, u32, u32)>> = Cell::new(None);
    }

    /// Reusable buffer for the combined panic message. Since `catch_panic` is
    /// synchronous and the caller copies the `&'static str` into an error
    /// variant before the next panic can occur, the old contents are always
    /// stale by the time we overwrite.
    struct SyncUnsafeCell(std::cell::UnsafeCell<String>);
    unsafe impl Sync for SyncUnsafeCell {}
    static PANIC_MSG_BUF: SyncUnsafeCell =
        SyncUnsafeCell(std::cell::UnsafeCell::new(String::new()));

    fn ensure_panic_hook_installed() {
        use std::sync::Once;
        static HOOK: Once = Once::new();
        HOOK.call_once(|| {
            // take_hook + forget prevents dropping the previous hook, which
            // may have been allocated by a now-unloaded cdylib.
            let prev = std::panic::take_hook();
            std::mem::forget(prev);
            std::panic::set_hook(Box::new(|info| {
                if let Some(loc) = info.location() {
                    PANIC_LOCATION.set(Some((
                        Box::leak(loc.file().to_string().into_boxed_str()),
                        loc.line(),
                        loc.column(),
                    )));
                }
            }));
        });
    }

    /// Runs a closure with `catch_unwind`, extracting the panic message as a
    /// `&'static str`. Uses `downcast_unchecked` + `size_of_val` to avoid
    /// `TypeId` checks that fail across cdylib boundaries.
    ///
    /// A panic hook is auto-installed on first call to capture location info
    /// (file:line:col) which is prepended to the message.
    pub fn catch_panic<R>(f: impl FnOnce() -> R) -> Result<R, &'static str> {
        ensure_panic_hook_installed();
        PANIC_LOCATION.set(None);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

        match result {
            Ok(value) => Ok(value),
            Err(err) => {
                // Discriminate by concrete type size via the vtable.
                // String = 3×usize (ptr, len, cap), &str = 2×usize (ptr, len).
                let payload: &'static str = unsafe {
                    if std::mem::size_of_val(&*err) == std::mem::size_of::<String>() {
                        let s = err.downcast_unchecked::<String>();
                        &*Box::leak(s)
                    } else {
                        let s = err.downcast_unchecked::<&str>();
                        let msg = *s;
                        std::mem::forget(s);
                        msg
                    }
                };

                // Combine location + message into the static buffer.
                // Safety: catch_panic is synchronous and single-threaded.
                // The returned &'static str is turned into a String owned by the binary
                // and the &'static str is dropped
                // before the next call could overwrite the buffer.
                let msg = if let Some((file, line, col)) = PANIC_LOCATION.get() {
                    unsafe {
                        use std::fmt::Write;
                        let buf = &mut *PANIC_MSG_BUF.0.get();
                        buf.clear();
                        let _ = write!(buf, "panicked at {file}:{line}:{col}: {payload}");
                        buf.as_str()
                    }
                } else {
                    payload
                };

                Err(msg)
            }
        }
    }
}
