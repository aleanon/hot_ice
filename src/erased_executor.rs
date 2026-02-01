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
//! +-----------------------+            +-----------------------+
//! | Reloader              |            | Worker Thread         |
//! |  update() called      |            |   executor.enter(||{  |
//! |  -> task = app.update |            |     block_on(async {  |
//! |  -> sync-drain Ready  |            |       poll streams    |
//! |  -> send Pending -----+----------->|       via channel     |
//! |     stream to worker  |            |     })                |
//! |                       |            |   })                  |
//! |  Actions arrive back  |            |                       |
//! |  via Proxy (event loop|<-----------+-- proxy.send_action() |
//! |  UserEvent)           |            |                       |
//! +-----------------------+            +-----------------------+
//! ```
//!
//! # How it works
//!
//! 1. On library load, the cdylib creates an executor and spawns a worker thread
//! 2. The worker thread enters the executor's TLS context via `Executor::enter()`
//! 3. A callback into the main binary runs a polling loop using `FuturesUnordered`
//! 4. The main binary sends `BoxStream<Action<Message>>` to the worker via channel
//! 5. The worker polls streams and sends resulting actions back via `Proxy::send_action()`
//! 6. On library unload, a shutdown command stops the worker and the thread is joined

use std::any::Any;
use std::sync::{Arc, OnceLock};

use crate::lib_reloader::LibReloader;
use crate::winit::Proxy;

// ---------------------------------------------------------------------------
// Global proxy storage — set by winit/mod.rs::run(), read by Reloader::new()
// ---------------------------------------------------------------------------

/// Type-erased global proxy for the event loop.
///
/// Set once during `winit::run()` initialization, before `Program::boot()`.
/// Retrieved by `Reloader::new()` to give workers a way to send actions
/// back to the event loop.
static GLOBAL_PROXY: OnceLock<Arc<dyn Any + Send + Sync>> = OnceLock::new();

/// Stores a clone of the event loop proxy for later retrieval.
///
/// Called once during `winit::run()` before the program is booted.
pub fn set_global_proxy<M: Send + 'static>(proxy: Proxy<M>) {
    let _ = GLOBAL_PROXY.set(Arc::new(proxy));
}

/// Retrieves a clone of the stored event loop proxy.
///
/// Returns `None` if `set_global_proxy` hasn't been called yet or if
/// the type parameter doesn't match.
pub fn get_global_proxy<M: Send + 'static>() -> Option<Proxy<M>> {
    GLOBAL_PROXY.get()?.downcast_ref::<Proxy<M>>().cloned()
}

// ---------------------------------------------------------------------------
// FFI function pointer types
// ---------------------------------------------------------------------------

/// Function pointer types for the worker FFI interface.
///
/// These are the signatures of functions exported from the cdylib via
/// the `export_executor!` macro.
pub mod ffi {
    /// Starts a worker thread inside the cdylib.
    ///
    /// The cdylib creates an executor, spawns a thread that enters the
    /// executor's TLS context, and calls `run_fn(data)` on that thread.
    ///
    /// - `data`: opaque pointer to a `WorkerContext` created by the main binary
    /// - `run_fn`: callback function defined in the main binary (the polling loop)
    ///
    /// Returns an opaque handle for stopping the worker, or null on failure.
    pub type StartWorkerFn =
        unsafe extern "C" fn(data: *mut (), run_fn: unsafe extern "C" fn(*mut ())) -> *mut ();

    /// Stops the worker thread and joins it.
    ///
    /// The handle was returned by `StartWorkerFn`. The main binary should
    /// have already sent a shutdown command via the channel so the worker
    /// loop exits before this is called.
    pub type StopWorkerFn = unsafe extern "C" fn(handle: *mut ());
}

// ---------------------------------------------------------------------------
// Worker channel protocol
// ---------------------------------------------------------------------------

use futures::stream::BoxStream;
use iced_runtime::Action;

/// Command sent from the main thread to the cdylib worker thread.
pub enum WorkerCommand<M> {
    /// Poll this stream to completion, forwarding actions via the proxy.
    RunStream(BoxStream<'static, Action<M>>),
    /// Shut down the worker thread immediately, dropping in-flight streams.
    Shutdown,
    /// Stop accepting new streams and drain active ones to completion,
    /// with a timeout to prevent indefinite blocking.
    Drain { timeout: std::time::Duration },
}

// ---------------------------------------------------------------------------
// Worker context and trampoline
// ---------------------------------------------------------------------------

use futures::channel::mpsc as fmpsc;

/// Context for the worker polling loop. Allocated on the main binary's heap
/// and passed to the cdylib as an opaque `*mut ()`.
///
/// The generic `M` (message type) is known to the main binary but invisible
/// to the cdylib — all the cdylib does is call `run_fn(data)`.
struct WorkerContext<M: Send + 'static> {
    /// Receives stream commands from the main thread.
    command_rx: fmpsc::UnboundedReceiver<WorkerCommand<M>>,
    /// Proxy to send actions back to the event loop.
    proxy: Proxy<M>,
}

/// The polling loop that runs on the cdylib's worker thread.
///
/// This is an `unsafe extern "C" fn` so it can be passed as a function pointer
/// across FFI. It is monomorphized in the main binary (knows `M`), but
/// executed on the cdylib's thread inside `executor.enter()`.
///
/// Uses `futures::executor::block_on` (NOT tokio's) to drive a
/// `FuturesUnordered` that concurrently polls all active streams.
/// `block_on` does not set up any TLS — the cdylib's `executor.enter()`
/// already handles that. So `tokio::spawn()` calls inside the streams
/// find the runtime handle in TLS and work correctly.
///
/// # Safety
///
/// `data` must point to a valid `Box<WorkerContext<M>>` that was created
/// by `CdylibWorker::start()`.
unsafe extern "C" fn worker_trampoline<M: Send + 'static>(data: *mut ()) {
    // Take ownership of the context
    let ctx = unsafe { Box::from_raw(data as *mut WorkerContext<M>) };

    futures::executor::block_on(worker_loop(ctx.command_rx, ctx.proxy));
}

/// The async polling loop driven by `block_on`.
///
/// Concurrently polls all active streams using `FuturesUnordered` while
/// accepting new stream commands from the channel.
async fn worker_loop<M: Send + 'static>(
    mut command_rx: fmpsc::UnboundedReceiver<WorkerCommand<M>>,
    proxy: Proxy<M>,
) {
    use futures::stream::{FuturesUnordered, StreamExt};

    let mut active: FuturesUnordered<futures::future::BoxFuture<'static, ()>> =
        FuturesUnordered::new();

    // Seed with a pending future so FuturesUnordered never returns None
    // (which would mean "empty" and terminate the select loop).
    // We use a future that never completes.
    active.push(Box::pin(futures::future::pending()));

    loop {
        futures::select! {
            // Poll all active stream-draining futures
            _ = active.select_next_some() => {
                // A stream finished draining — nothing to do, it's removed
                // from FuturesUnordered automatically.
            }
            // Check for new commands
            cmd = command_rx.select_next_some() => {
                match cmd {
                    WorkerCommand::RunStream(stream) => {
                        let proxy = proxy.clone();
                        active.push(Box::pin(drain_stream(stream, proxy)));
                    }
                    WorkerCommand::Shutdown => {
                        break;
                    }
                    WorkerCommand::Drain { timeout } => {
                        drain_active(&mut active, timeout).await;
                        return;
                    }
                }
            }
            // Both channels closed
            complete => break,
        }
    }
}

/// Polls all active futures to completion, with a timeout.
///
/// The `FuturesUnordered` contains N real tasks + 1 sentinel `pending()`
/// future. When only the sentinel remains (`len() <= 1`), all real work
/// is done.
async fn drain_active(
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

/// Drains a stream to completion, sending each action to the event loop.
async fn drain_stream<M: 'static>(stream: BoxStream<'static, Action<M>>, proxy: Proxy<M>) {
    use futures::StreamExt;
    futures::pin_mut!(stream);
    while let Some(action) = stream.next().await {
        proxy.send_action(action);
    }
}

// ---------------------------------------------------------------------------
// CdylibWorker — main binary's handle to the worker thread
// ---------------------------------------------------------------------------

/// Handle to a running worker thread inside a cdylib.
///
/// Owns the command channel sender and the FFI handle for stopping.
/// When dropped or shut down, the worker thread is joined.
pub struct CdylibWorker<M: Send + 'static> {
    /// Send stream commands to the worker thread.
    command_tx: fmpsc::UnboundedSender<WorkerCommand<M>>,
    /// FFI function to stop the worker (joins thread).
    stop_fn: ffi::StopWorkerFn,
    /// Opaque handle returned by `start_worker`, passed to `stop_worker`.
    worker_handle: *mut (),
}

// Safety: The worker_handle is only used to call stop_fn, which joins the
// thread. The channel sender is Send. The FFI function pointers are valid
// for the library's lifetime (managed by LibReloader/RetiredLibrary).
unsafe impl<M: Send> Send for CdylibWorker<M> {}
unsafe impl<M: Send> Sync for CdylibWorker<M> {}

impl<M: Send + 'static> CdylibWorker<M> {
    /// Starts a new worker from the loaded library.
    ///
    /// Loads the `start_worker` and `stop_worker` FFI symbols, creates the
    /// communication channels, and calls into the cdylib to spawn the worker
    /// thread.
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

        let ctx = Box::new(WorkerContext { command_rx, proxy });
        let ctx_ptr = Box::into_raw(ctx) as *mut ();

        let worker_handle = unsafe { start_fn(ctx_ptr, worker_trampoline::<M>) };

        if worker_handle.is_null() {
            // Reclaim context to avoid leak
            unsafe {
                let _ = Box::from_raw(ctx_ptr as *mut WorkerContext<M>);
            }
            return Err("start_worker returned null".into());
        }

        Ok(Self {
            command_tx,
            stop_fn,
            worker_handle,
        })
    }

    /// Submits a stream for the worker to poll to completion.
    ///
    /// Actions produced by the stream are sent back to the event loop
    /// via `Proxy::send_action()`.
    pub fn run_stream(&self, stream: BoxStream<'static, Action<M>>) {
        let _ = self
            .command_tx
            .unbounded_send(WorkerCommand::RunStream(stream));
    }

    /// Shuts down the worker thread.
    ///
    /// Sends a shutdown command, then calls the cdylib's `stop_worker` FFI
    /// function which joins the thread.
    pub fn shutdown(&mut self) {
        // Send shutdown command (ignore error if already disconnected)
        let _ = self.command_tx.unbounded_send(WorkerCommand::Shutdown);
        // Close the channel so the worker sees disconnection
        self.command_tx.close_channel();
        // Join the worker thread via FFI
        if !self.worker_handle.is_null() {
            unsafe {
                (self.stop_fn)(self.worker_handle);
            }
            self.worker_handle = std::ptr::null_mut();
        }
    }

    /// Begins a graceful drain of all active streams, then returns a
    /// [`DrainHandle`] that the caller must eventually join.
    ///
    /// Sends a `Drain` command with the given timeout, closes the channel,
    /// and returns a handle. The caller should call `DrainHandle::join()` on
    /// a background thread — it blocks until the worker exits (drain
    /// completes or times out).
    ///
    /// After calling this, the `CdylibWorker` is consumed and cannot be used.
    pub fn begin_drain(mut self, timeout: std::time::Duration) -> DrainHandle {
        let _ = self
            .command_tx
            .unbounded_send(WorkerCommand::Drain { timeout });
        self.command_tx.close_channel();

        let handle = DrainHandle {
            stop_fn: self.stop_fn,
            worker_handle: self.worker_handle,
        };

        // Prevent Drop from calling shutdown
        self.worker_handle = std::ptr::null_mut();
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
pub struct DrainHandle {
    stop_fn: ffi::StopWorkerFn,
    worker_handle: *mut (),
}

// Safety: same reasoning as CdylibWorker — worker_handle is only used to
// call stop_fn which joins the thread.
unsafe impl Send for DrainHandle {}
unsafe impl Sync for DrainHandle {}

impl DrainHandle {
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
    }
}

impl Drop for DrainHandle {
    fn drop(&mut self) {
        if !self.worker_handle.is_null() {
            log::warn!("hot-ice drain: DrainHandle dropped without join(), joining now");
            unsafe {
                (self.stop_fn)(self.worker_handle);
            }
            self.worker_handle = std::ptr::null_mut();
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
        $crate::export_executor!($crate::iced_futures::backend::default::Executor);
    };
    ($executor_ty:ty) => {
        /// Starts a worker thread inside this cdylib.
        ///
        /// Creates an executor, spawns a thread that enters the executor's
        /// TLS context, and calls `run_fn(data)` on that thread.
        /// Returns an opaque handle for `stop_worker` to join.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn start_worker_lskdjfa3lkfjasdf(
            data: *mut (),
            run_fn: unsafe extern "C" fn(*mut ()),
        ) -> *mut () {
            let executor = match <$executor_ty as $crate::iced_futures::Executor>::new() {
                Ok(e) => e,
                Err(err) => {
                    ::std::eprintln!("hot_ice: failed to create executor in cdylib: {}", err);
                    return ::std::ptr::null_mut();
                }
            };

            // Move executor into an Arc so both the thread and the handle can reference it.
            // The thread needs it for enter(); the handle keeps it alive until join.
            let executor = ::std::sync::Arc::new(executor);
            let exec_for_thread = executor.clone();

            // Bundle the FFI callback data into a Send wrapper so it can
            // cross the thread::spawn boundary.
            //
            // Bundle the FFI callback into a Send wrapper that also serves
            // as a callable. The wrapper stays intact through spawn and enter,
            // so the compiler never sees bare `*mut ()` in any closure capture.
            //
            // Safety: `data` points to a WorkerContext (Send) allocated by the
            // main binary, and `run_fn` is a plain function pointer (inherently
            // thread-safe).
            struct FfiCallback {
                data: *mut (),
                run_fn: unsafe extern "C" fn(*mut ()),
            }
            unsafe impl Send for FfiCallback {}
            impl FfiCallback {
                unsafe fn call(self) {
                    unsafe {
                        (self.run_fn)(self.data);
                    }
                }
            }

            let callback = FfiCallback { data, run_fn };

            let join_handle = ::std::thread::Builder::new()
                .name("hot-ice-worker".into())
                .spawn(move || {
                    // Enter the executor's TLS context on this dedicated thread.
                    // All tokio::spawn() calls inside the callback will find the
                    // runtime handle in this thread's TLS.
                    <$executor_ty as $crate::iced_futures::Executor>::enter(
                        &exec_for_thread,
                        move || {
                            // Call back into the main binary's worker loop
                            unsafe {
                                callback.call();
                            }
                        },
                    );
                    // executor Arc ref dropped here
                })
                .expect("hot_ice: failed to spawn cdylib worker thread");

            let handle = ::std::boxed::Box::new((join_handle, executor));
            ::std::boxed::Box::into_raw(handle) as *mut ()
        }

        /// Stops the worker thread by joining it.
        ///
        /// The main binary should have already sent a shutdown command via
        /// the channel so `run_fn` returns before this is called.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn stop_worker_lskdjfa3lkfjasdf(handle: *mut ()) {
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
            // _executor Arc dropped here — if this was the last ref, the
            // executor (e.g. tokio runtime) is shut down
        }
    };
}
