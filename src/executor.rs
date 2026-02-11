//! Cdylib worker thread for hot-reloadable libraries.
//!
//! This module provides a dedicated worker thread that runs inside the cdylib's
//! executor context (TLS), allowing `tokio::spawn()` and similar runtime calls
//! to work correctly from hot-reloaded code.
//!
//! # Architecture
//!
//! ```text
//! Main Binary                          cdylib (hot-reloaded)
//! +-----------------------+            +-------------------------------+
//! | CdylibWorker<M>       |            | export_executor! macro code   |
//! |  erases Action<M>     |            |   spawns thread               |
//! |  → *mut () items      |            |   executor.enter(||{          |
//! |  sends erased cmds ───+──────────→ |     erased_worker_loop(ctx)   |
//! |                       |            |       polls erased streams    |
//! |  action_callback<M>() |            |       catch_unwind per stream |
//! |  ← *mut () action ───←+←────────← |       calls action_callback   |
//! |  unbox → Action<M>    |            |       calls panic_callback    |
//! |  proxy.send_action()  |            |   })                         |
//! +-----------------------+            +-------------------------------+
//! ```
//!
//! # How it works
//!
//! 1. On library load, the cdylib creates an executor and spawns a worker thread
//! 2. The worker thread enters the executor's TLS context via `Executor::enter()`
//! 3. The cdylib runs the type-erased polling loop (`erased_worker_loop`)
//! 4. The main binary sends type-erased streams to the worker via a channel
//! 5. The worker polls streams; each action item is forwarded via a callback
//!    that reconstructs `Action<M>` and calls `Proxy::send_action()`
//! 6. Panics in user async code are caught by `catch_unwind` inside the cdylib
//! 7. On library unload, a shutdown command stops the worker and the thread is joined

use std::any::Any;
use std::sync::{Arc, OnceLock};

use crate::lib_reloader::LibReloader;
use crate::winit::Proxy;

/// Wrapper around `*mut ()` that implements `Send + Sync`.
///
/// Used to pass raw callback pointers into async contexts.
/// Safety: the pointed-to data must actually be safe to access from the
/// worker thread (which it is — `CallbackContext<M>` is only accessed
/// sequentially from the single worker thread).
#[derive(Clone, Copy)]
struct SendPtr(*mut ());
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

impl SendPtr {
    fn as_ptr(self) -> *mut () {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Global proxy storage — set by winit/mod.rs::run(), read by Reloader::new()
// ---------------------------------------------------------------------------

static GLOBAL_PROXY: OnceLock<Arc<dyn Any + Send + Sync>> = OnceLock::new();

pub fn set_global_proxy<M: Send + 'static>(proxy: Proxy<M>) {
    let _ = GLOBAL_PROXY.set(Arc::new(proxy));
}

pub fn get_global_proxy<M: Send + 'static>() -> Option<Proxy<M>> {
    GLOBAL_PROXY.get()?.downcast_ref::<Proxy<M>>().cloned()
}

// ---------------------------------------------------------------------------
// FFI function pointer types
// ---------------------------------------------------------------------------

pub mod ffi {
    /// Starts a worker thread inside the cdylib.
    ///
    /// `ctx` is a `*mut ErasedWorkerContext` created by the main binary.
    /// The cdylib takes ownership, spawns a thread with executor TLS,
    /// and runs the polling loop.
    ///
    /// Returns an opaque handle for `stop_worker`, or null on failure.
    pub type StartWorkerFn = unsafe fn(ctx: *mut ()) -> *mut ();

    /// Stops the worker thread and joins it.
    pub type StopWorkerFn = unsafe fn(handle: *mut ());
}

// ---------------------------------------------------------------------------
// Type-erased worker protocol
// ---------------------------------------------------------------------------

use futures::channel::mpsc as fmpsc;
use futures::stream::BoxStream;
use iced_runtime::Action;

/// Type-erased stream: each item is `Box::into_raw(Box::new(Action<M>))`.
pub type ErasedStream = BoxStream<'static, *mut ()>;

/// Called by the cdylib for each action pointer produced by a stream.
/// The main binary reconstructs `Action<M>` and calls `proxy.send_action()`.
pub type ActionCallbackFn = unsafe fn(ctx: *mut (), action_ptr: *mut ());

/// Called by the cdylib when a stream panics.
/// The main binary receives the panic message as a UTF-8 byte slice.
pub type PanicCallbackFn = unsafe fn(ctx: *mut (), msg_ptr: *const u8, msg_len: usize);

/// Non-generic command sent over the channel from main binary to worker.
pub enum ErasedWorkerCommand {
    /// Poll this type-erased stream to completion.
    RunStream(ErasedStream),
    /// Shut down the worker thread immediately.
    Shutdown,
    /// Drain active streams with a timeout, then exit.
    Drain { timeout: std::time::Duration },
}

/// Non-generic context passed from the main binary to the cdylib.
///
/// The cdylib's `start_worker` function receives this as `*mut ()`,
/// reconstructs it, and passes it to `erased_worker_loop`.
pub struct ErasedWorkerContext {
    pub command_rx: fmpsc::UnboundedReceiver<ErasedWorkerCommand>,
    pub callback_ctx: *mut (),
    pub action_callback: ActionCallbackFn,
    pub panic_callback: PanicCallbackFn,
}

// Safety: command_rx is Send, callback_ctx points to a Send type
// (CallbackContext<M>), function pointers are inherently thread-safe.
unsafe impl Send for ErasedWorkerContext {}

// ---------------------------------------------------------------------------
// Erased polling loop — compiled into the cdylib via hot_ice dependency
// ---------------------------------------------------------------------------

/// Entry point for the type-erased polling loop.
///
/// This function is non-generic and compiles into the cdylib (via the
/// `hot_ice` dependency). Panics from user async code are caught here,
/// in the same compilation unit as the user's code.
pub fn erased_worker_loop(ctx: ErasedWorkerContext) {
    crate::panic_hook::ensure_panic_hook_installed();

    let ErasedWorkerContext {
        command_rx,
        callback_ctx,
        action_callback,
        panic_callback,
    } = ctx;

    futures::executor::block_on(erased_worker_loop_async(
        command_rx,
        callback_ctx,
        action_callback,
        panic_callback,
    ));
}

async fn erased_worker_loop_async(
    mut command_rx: fmpsc::UnboundedReceiver<ErasedWorkerCommand>,
    callback_ctx: *mut (),
    action_callback: ActionCallbackFn,
    panic_callback: PanicCallbackFn,
) {
    use futures::FutureExt;
    use futures::stream::{FuturesUnordered, StreamExt};
    use std::panic::AssertUnwindSafe;

    let ctx = SendPtr(callback_ctx);

    let mut active: FuturesUnordered<futures::future::BoxFuture<'static, ()>> =
        FuturesUnordered::new();

    // Sentinel future that never completes — prevents FuturesUnordered
    // from returning None when empty.
    active.push(Box::pin(futures::future::pending()));

    loop {
        let result = AssertUnwindSafe(async {
            loop {
                futures::select! {
                    _ = active.select_next_some() => {
                        // A stream finished draining — removed automatically.
                    }
                    cmd = command_rx.select_next_some() => {
                        match cmd {
                            ErasedWorkerCommand::RunStream(stream) => {
                                let cb_ctx = ctx;
                                let action_cb = action_callback;
                                let panic_cb = panic_callback;

                                active.push(Box::pin(
                                    AssertUnwindSafe(
                                        erased_drain_stream(stream, cb_ctx, action_cb),
                                    )
                                    .catch_unwind()
                                    .map(move |result| {
                                        handle_stream_result(result, cb_ctx, panic_cb);
                                    }),
                                ));
                            }
                            ErasedWorkerCommand::Shutdown => {
                                break;
                            }
                            ErasedWorkerCommand::Drain { timeout } => {
                                erased_drain_active(&mut active, timeout).await;
                                return;
                            }
                        }
                    }
                    complete => break,
                }
            }
        })
        .catch_unwind()
        .await;

        if let Err(panic) = result {
            // Outer catch_unwind: keeps the worker alive if the select!
            // machinery itself panics. Forget the payload to avoid
            // cross-cdylib drop issues.
            std::mem::forget(panic);
        }
    }
}

/// Handles the result of a catch_unwind around a drain_stream future.
/// Extracted into a separate function to avoid capturing `*mut ()` in async context.
fn handle_stream_result(
    result: Result<(), Box<dyn Any + Send>>,
    cb_ctx: SendPtr,
    panic_cb: PanicCallbackFn,
) {
    if let Err(ref panic) = result {
        let msg = extract_panic_message(panic);
        let bytes = msg.as_bytes();
        unsafe {
            panic_cb(cb_ctx.as_ptr(), bytes.as_ptr(), bytes.len());
        }
    }
    if let Err(panic) = result {
        std::mem::forget(panic);
    }
}

/// Drains a type-erased stream, calling the action callback for each item.
async fn erased_drain_stream(
    stream: ErasedStream,
    callback_ctx: SendPtr,
    action_callback: ActionCallbackFn,
) {
    use futures::StreamExt;
    futures::pin_mut!(stream);
    while let Some(action_ptr) = stream.next().await {
        unsafe {
            action_callback(callback_ctx.0, action_ptr);
        }
    }
}

/// Polls all active futures to completion, with a timeout.
async fn erased_drain_active(
    active: &mut futures::stream::FuturesUnordered<futures::future::BoxFuture<'static, ()>>,
    timeout: std::time::Duration,
) {
    use futures::FutureExt;
    use futures::stream::StreamExt;

    if active.len() <= 1 {
        log::info!("hot-ice drain: no active streams, exiting immediately");
        return;
    }

    log::info!(
        "hot-ice drain: waiting for {} active stream(s) to complete (timeout: {:?})",
        active.len() - 1,
        timeout,
    );

    let deadline = futures_timer::Delay::new(timeout).fuse();
    futures::pin_mut!(deadline);

    loop {
        futures::select! {
            _ = active.select_next_some() => {
                if active.len() <= 1 {
                    log::info!("hot-ice drain: all streams completed");
                    return;
                }
            }
            _ = deadline => {
                log::warn!(
                    "hot-ice drain: timeout after {:?}, dropping {} remaining stream(s)",
                    timeout,
                    active.len() - 1,
                );
                return;
            }
        }
    }
}

/// Extracts a human-readable message from a panic payload.
///
/// Uses the `size_of_val` trick (same as `catch_panic`) to discriminate
/// between `String` and `&str` payloads without `TypeId`.
fn extract_panic_message(err: &Box<dyn Any + Send>) -> &str {
    unsafe {
        if std::mem::size_of_val(&**err) == std::mem::size_of::<String>() {
            &*(&**err as *const dyn Any as *const String)
        } else if std::mem::size_of_val(&**err) == std::mem::size_of::<&str>() {
            *(&**err as *const dyn Any as *const &str)
        } else {
            "unknown panic"
        }
    }
}

// ---------------------------------------------------------------------------
// Main binary side: CallbackContext and callback implementations
// ---------------------------------------------------------------------------

/// Holds the generic state that callbacks need.
/// Allocated on the main binary's heap, passed to the cdylib as `*mut ()`.
struct CallbackContext<M: Send + 'static> {
    proxy: Proxy<M>,
}

/// Reconstructs `Action<M>` from the opaque pointer and sends it via proxy.
///
/// # Safety
///
/// `ctx` must point to a valid `CallbackContext<M>`.
/// `action_ptr` must point to a valid `Box<Action<M>>` created by stream erasure.
unsafe fn action_callback_impl<M: Send + 'static>(ctx: *mut (), action_ptr: *mut ()) {
    let cb_ctx = unsafe { &*(ctx as *const CallbackContext<M>) };
    let action = unsafe { *Box::from_raw(action_ptr as *mut Action<M>) };
    cb_ctx.proxy.send_action(action);
}

/// Receives a panic message from the cdylib and logs it.
///
/// # Safety
///
/// `msg_ptr` must point to valid UTF-8 bytes of length `msg_len`,
/// or be null (in which case a default message is used).
unsafe fn panic_callback_impl<M: Send + 'static>(
    _ctx: *mut (),
    msg_ptr: *const u8,
    msg_len: usize,
) {
    let msg = if msg_ptr.is_null() || msg_len == 0 {
        "unknown panic"
    } else {
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(msg_ptr, msg_len)) }
    };
    log::error!("hot-ice worker: stream panicked: {}", msg);
}

// ---------------------------------------------------------------------------
// CdylibWorker — main binary's handle to the worker thread
// ---------------------------------------------------------------------------

/// Handle to a running worker thread inside a cdylib.
///
/// Owns the command channel sender and the FFI handle for stopping.
/// When dropped or shut down, the worker thread is joined.
pub struct CdylibWorker<M: Send + 'static> {
    /// Send erased commands to the worker thread.
    command_tx: fmpsc::UnboundedSender<ErasedWorkerCommand>,
    /// FFI function to stop the worker (joins thread).
    stop_fn: ffi::StopWorkerFn,
    /// Opaque handle returned by `start_worker`, passed to `stop_worker`.
    worker_handle: *mut (),
    /// Pointer to the `CallbackContext<M>` on the heap.
    /// Freed after the worker thread is joined.
    callback_ctx_ptr: *mut (),
    /// Marker for the message type.
    _marker: std::marker::PhantomData<M>,
}

// Safety: The worker_handle and callback_ctx_ptr are only used through
// FFI calls and cleanup. The channel sender is Send.
unsafe impl<M: Send> Send for CdylibWorker<M> {}
unsafe impl<M: Send> Sync for CdylibWorker<M> {}

impl<M: Send + 'static> CdylibWorker<M> {
    /// Starts a new worker from the loaded library.
    ///
    /// Loads the `start_worker` and `stop_worker` FFI symbols, creates the
    /// communication channels with type-erased protocol, and calls into
    /// the cdylib to spawn the worker thread.
    ///
    /// # Safety
    ///
    /// The library must export `start_worker_*` and `stop_worker_*` symbols
    /// (generated by `export_executor!`).
    pub unsafe fn start(lib_reloader: &LibReloader, proxy: Proxy<M>) -> Result<Self, String> {
        let start_fn: ffi::StartWorkerFn = unsafe {
            *lib_reloader
                .get_symbol(hot_ice_common::START_WORKER_FUNCTION_NAME.as_bytes())
                .map_err(|e| format!("Failed to get start_worker: {}", e))?
        };
        let stop_fn: ffi::StopWorkerFn = unsafe {
            *lib_reloader
                .get_symbol(hot_ice_common::STOP_WORKER_FUNCTION_NAME.as_bytes())
                .map_err(|e| format!("Failed to get stop_worker: {}", e))?
        };

        let (command_tx, command_rx) = fmpsc::unbounded();

        // Allocate callback context on the heap
        let cb_ctx = Box::new(CallbackContext { proxy });
        let callback_ctx_ptr = Box::into_raw(cb_ctx) as *mut ();

        // Create the erased worker context
        let ctx = Box::new(ErasedWorkerContext {
            command_rx,
            callback_ctx: callback_ctx_ptr,
            action_callback: action_callback_impl::<M>,
            panic_callback: panic_callback_impl::<M>,
        });
        let ctx_ptr = Box::into_raw(ctx) as *mut ();

        let worker_handle = unsafe { start_fn(ctx_ptr) };

        if worker_handle.is_null() {
            // Reclaim to avoid leaks
            unsafe {
                let _ = Box::from_raw(ctx_ptr as *mut ErasedWorkerContext);
                let _ = Box::from_raw(callback_ctx_ptr as *mut CallbackContext<M>);
            }
            return Err("start_worker returned null".into());
        }

        Ok(Self {
            command_tx,
            stop_fn,
            worker_handle,
            callback_ctx_ptr,
            _marker: std::marker::PhantomData,
        })
    }

    /// Submits a stream for the worker to poll to completion.
    ///
    /// The stream is type-erased: each `Action<M>` is boxed and converted
    /// to `*mut ()`. The cdylib's polling loop forwards each pointer back
    /// via the action callback, which reconstructs and delivers it.
    pub fn run_stream(&self, stream: BoxStream<'static, Action<M>>) {
        use futures::StreamExt;
        let erased: ErasedStream =
            Box::pin(stream.map(|action| Box::into_raw(Box::new(action)) as *mut ()));
        let _ = self
            .command_tx
            .unbounded_send(ErasedWorkerCommand::RunStream(erased));
    }

    /// Shuts down the worker thread.
    ///
    /// Sends a shutdown command, then calls the cdylib's `stop_worker` FFI
    /// function which joins the thread. Finally frees the callback context.
    pub fn shutdown(&mut self) {
        let _ = self
            .command_tx
            .unbounded_send(ErasedWorkerCommand::Shutdown);
        self.command_tx.close_channel();

        if !self.worker_handle.is_null() {
            unsafe {
                (self.stop_fn)(self.worker_handle);
            }
            self.worker_handle = std::ptr::null_mut();
        }

        // Free the callback context now that the worker is joined
        if !self.callback_ctx_ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(self.callback_ctx_ptr as *mut CallbackContext<M>);
            }
            self.callback_ctx_ptr = std::ptr::null_mut();
        }
    }

    /// Begins a graceful drain of all active streams, then returns a
    /// [`DrainHandle`] that the caller must eventually join.
    ///
    /// Sends a `Drain` command with the given timeout, closes the channel,
    /// and returns a handle. The caller should call `DrainHandle::join()` on
    /// a background thread — it blocks until the worker exits (drain
    /// completes or times out).
    pub fn begin_drain(mut self, timeout: std::time::Duration) -> DrainHandle<M> {
        let _ = self
            .command_tx
            .unbounded_send(ErasedWorkerCommand::Drain { timeout });
        self.command_tx.close_channel();

        let handle = DrainHandle {
            stop_fn: self.stop_fn,
            worker_handle: self.worker_handle,
            callback_ctx_ptr: self.callback_ctx_ptr,
            _marker: std::marker::PhantomData::<M>,
        };

        // Prevent Drop from calling shutdown
        self.worker_handle = std::ptr::null_mut();
        self.callback_ctx_ptr = std::ptr::null_mut();
        std::mem::forget(self);

        handle
    }
}

impl<M: Send + 'static> Drop for CdylibWorker<M> {
    fn drop(&mut self) {
        if !self.worker_handle.is_null() {
            self.shutdown();
        }
    }
}

// ---------------------------------------------------------------------------
// DrainHandle — background cleanup handle for a draining worker
// ---------------------------------------------------------------------------

/// Opaque handle for joining a draining worker thread.
///
/// Returned by [`CdylibWorker::begin_drain()`]. The caller must eventually
/// call [`join()`](DrainHandle::join) to block until the worker thread exits.
/// The `Drop` impl calls `join` as a safety net if the caller forgets.
pub struct DrainHandle<M: Send + 'static> {
    stop_fn: ffi::StopWorkerFn,
    worker_handle: *mut (),
    callback_ctx_ptr: *mut (),
    _marker: std::marker::PhantomData<M>,
}

unsafe impl<M: Send> Send for DrainHandle<M> {}
unsafe impl<M: Send> Sync for DrainHandle<M> {}

impl<M: Send + 'static> DrainHandle<M> {
    /// Blocks until the worker thread exits (drain completes or times out).
    pub fn join(mut self) {
        if !self.worker_handle.is_null() {
            log::info!("hot-ice drain: joining old worker thread");
            unsafe {
                (self.stop_fn)(self.worker_handle);
            }
            self.worker_handle = std::ptr::null_mut();
            log::info!("hot-ice drain: old worker thread joined");
        }
        if !self.callback_ctx_ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(self.callback_ctx_ptr as *mut CallbackContext<M>);
            }
            self.callback_ctx_ptr = std::ptr::null_mut();
        }
    }
}

impl<M: Send + 'static> Drop for DrainHandle<M> {
    fn drop(&mut self) {
        if !self.worker_handle.is_null() {
            log::warn!("hot-ice drain: DrainHandle dropped without join(), joining now");
            unsafe {
                (self.stop_fn)(self.worker_handle);
            }
            self.worker_handle = std::ptr::null_mut();
        }
        if !self.callback_ctx_ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(self.callback_ctx_ptr as *mut CallbackContext<M>);
            }
            self.callback_ctx_ptr = std::ptr::null_mut();
        }
    }
}

// ============================================================================
// Cdylib-side: export_executor! macro
// ============================================================================

/// Generates the `#[unsafe(no_mangle)]` FFI functions that the main binary
/// loads from the cdylib to start and stop a worker thread with the correct
/// executor TLS context.
///
/// The default invocation uses `iced_futures::backend::default::Executor`,
/// which matches whatever the user has enabled via cargo features
/// (tokio, smol, thread-pool, etc.).
///
/// # Custom executor
///
/// If the user calls `.executor::<MyExecutor>()` on the application builder,
/// they must also invoke this macro with the same type in their cdylib crate:
///
/// ```rust,ignore
/// hot_ice::export_executor!(MyExecutor);
/// ```
#[macro_export]
macro_rules! export_executor {
    () => {
        $crate::export_executor!($crate::macro_use::iced_futures::backend::default::Executor);
    };
    ($executor_ty:ty) => {
        /// Starts a worker thread inside this cdylib.
        ///
        /// Receives an `ErasedWorkerContext` as `*mut ()`, creates an executor,
        /// spawns a thread that enters the executor's TLS context, and runs
        /// the type-erased polling loop.
        #[unsafe(no_mangle)]
        pub unsafe fn start_worker_lskdjfa3lkfjasdf(ctx_ptr: *mut ()) -> *mut () {
            let executor = match <$executor_ty as $crate::macro_use::iced_futures::Executor>::new()
            {
                Ok(e) => e,
                Err(err) => {
                    ::std::eprintln!("hot_ice: failed to create executor in cdylib: {}", err);
                    return ::std::ptr::null_mut();
                }
            };

            let executor = ::std::sync::Arc::new(executor);
            let exec_for_thread = executor.clone();

            // Reconstruct the Box<ErasedWorkerContext> from the raw pointer.
            // ErasedWorkerContext is Send (unsafe impl), so the Box can
            // cross the thread::spawn boundary without a wrapper.
            let ctx_box: ::std::boxed::Box<$crate::executor::ErasedWorkerContext> = unsafe {
                ::std::boxed::Box::from_raw(ctx_ptr as *mut $crate::executor::ErasedWorkerContext)
            };

            let join_handle = ::std::thread::Builder::new()
                .name("hot-ice-worker".into())
                .spawn(move || {
                    let worker_ctx = *ctx_box;
                    <$executor_ty as $crate::macro_use::iced_futures::Executor>::enter(
                        &exec_for_thread,
                        move || {
                            $crate::executor::erased_worker_loop(worker_ctx);
                        },
                    );
                })
                .expect("hot_ice: failed to spawn cdylib worker thread");

            let handle = ::std::boxed::Box::new((join_handle, executor));
            ::std::boxed::Box::into_raw(handle) as *mut ()
        }

        /// Stops the worker thread by joining it.
        #[unsafe(no_mangle)]
        pub unsafe fn stop_worker_lskdjfa3lkfjasdf(handle: *mut ()) {
            if handle.is_null() {
                return;
            }
            let handle = unsafe {
                ::std::boxed::Box::from_raw(
                    handle
                        as *mut (
                            ::std::thread::JoinHandle<()>,
                            ::std::sync::Arc<$executor_ty>,
                        ),
                )
            };
            let (join_handle, _executor) = *handle;
            if let Err(err) = join_handle.join() {
                ::std::eprintln!("hot_ice: worker thread panicked: {:?}", err);
            }
        }
    };
}
