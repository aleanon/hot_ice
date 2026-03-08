use std::cell::Cell;
use std::sync::Mutex;

thread_local! {
    static PANIC_LOCATION: Cell<Option<(String, u32, u32)>> = const { Cell::new(None) };
}

/// Reusable buffer for the combined panic message. Protected by a Mutex
/// to prevent data races if two threads panic simultaneously.
static PANIC_MSG_BUF: Mutex<String> = Mutex::new(String::new());

pub(crate) fn ensure_panic_hook_installed() {
    use std::sync::Once;
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        // take_hook + forget prevents dropping the previous hook, which
        // may have been allocated by a now-unloaded cdylib.
        let prev = std::panic::take_hook();
        std::mem::forget(prev);
        std::panic::set_hook(Box::new(|info| {
            if let Some(loc) = info.location() {
                // Store the file as an owned String instead of Box::leak to avoid
                // memory leaks on every panic.
                PANIC_LOCATION.set(Some((
                    loc.file().to_string(),
                    loc.line(),
                    loc.column(),
                )));
            }
        }));
    });
}

/// Runs a closure with `catch_unwind`, extracting the panic message as a
/// `&'static str`. Uses safe `downcast` to discriminate between `String`
/// and `&str` payloads.
///
/// A panic hook is auto-installed on first call to capture location info
/// (file:line:col) which is prepended to the message.
pub fn catch_panic<R>(f: impl FnOnce() -> R) -> Result<R, &'static str> {
    ensure_panic_hook_installed();
    PANIC_LOCATION.set(None);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

    match result {
        Ok(value) => Ok(value),
        Err(err) => Err(extract_and_format_message(err)),
    }
}

/// Extracts the panic message, formats it with location info, and returns
/// a `&'static str`. The returned reference points into a global buffer
/// that is overwritten on the next call.
fn extract_and_format_message(err: Box<dyn std::any::Any + Send>) -> &'static str {
    // Try to extract String first, then &str. downcast() consumes the
    // Box on success and returns Err(original) on failure, so chaining
    // is safe and avoids cross-cdylib TypeId issues with downcast_ref.
    let payload: String = match err.downcast::<String>() {
        Ok(s) => *s,
        Err(err) => match err.downcast::<&str>() {
            Ok(s) => (*s).to_string(),
            Err(_) => "unknown panic".to_string(),
        },
    };

    // Combine location + message into the static buffer.
    if let Ok(mut buf) = PANIC_MSG_BUF.lock() {
        use std::fmt::Write;
        buf.clear();

        if let Some((file, line, col)) = PANIC_LOCATION.take() {
            let _ = write!(buf, "panicked at {file}:{line}:{col}: {payload}");
        } else {
            buf.push_str(&payload);
        }

        // Safety: catch_panic is synchronous and single-threaded per caller.
        // The returned &'static str reference is consumed (copied into an
        // owned String) by the caller before the next call can overwrite
        // the buffer. The 'static lifetime is a necessary lie for the API
        // contract — the buffer outlives any individual call.
        unsafe { &*(buf.as_str() as *const str) }
    } else {
        // Mutex poisoned — fall back to a static string.
        "unknown panic (mutex poisoned)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catch_panic_ok_result() {
        let result = catch_panic(|| 42);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn catch_panic_string_message() {
        let result = catch_panic(|| panic!("test panic message"));
        let err = result.unwrap_err();
        assert!(err.contains("test panic message"), "got: {err}");
    }

    #[test]
    fn catch_panic_str_message() {
        let result = catch_panic(|| {
            std::panic::panic_any("static str panic");
        });
        let err = result.unwrap_err();
        assert!(err.contains("static str panic"), "got: {err}");
    }

    #[test]
    fn catch_panic_unknown_type() {
        let result = catch_panic(|| {
            std::panic::panic_any(123i32);
        });
        let err = result.unwrap_err();
        assert!(err.contains("unknown panic"), "got: {err}");
    }

    #[test]
    fn catch_panic_includes_location() {
        let result = catch_panic(|| panic!("located panic"));
        let err = result.unwrap_err();
        // Should contain file:line:col prefix
        assert!(err.contains("panicked at"), "got: {err}");
        assert!(err.contains("located panic"), "got: {err}");
    }
}
