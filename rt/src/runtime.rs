mod byte_array;
mod class;
mod env;
mod float;
mod fs;
mod general;
mod helpers;
mod int;
mod process;
mod random;
mod signal;
mod socket;
mod stdio;
mod string;
mod sys;
mod time;
mod tls;

use crate::config::Config;
use crate::mem::ClassPointer;
use crate::network_poller::Worker as NetworkPollerWorker;
use crate::process::{NativeAsyncMethod, Process};
use crate::scheduler::reset_affinity;
use crate::scheduler::signal as signal_sched;
use crate::stack::total_stack_size;
use crate::stack::Stack;
use crate::state::{MethodCounts, RcState, State};
use rustix::param::page_size;
use std::ffi::CStr;
use std::io::{stdout, Write as _};
use std::process::exit as rust_exit;
use std::slice;
use std::thread;

#[no_mangle]
pub unsafe extern "system" fn inko_runtime_new(
    counts: *mut MethodCounts,
    argc: u32,
    argv: *const *const i8,
) -> *mut Runtime {
    // The first argument is the executable. Rust already supports fetching this
    // for us on all platforms, so we just discard it here and spare us having
    // to deal with any platform specifics.
    let mut args = Vec::with_capacity(argc as usize);

    if !argv.is_null() {
        for &ptr in slice::from_raw_parts(argv, argc as usize).iter().skip(1) {
            if ptr.is_null() {
                break;
            }

            args.push(CStr::from_ptr(ptr as _).to_string_lossy().into_owned());
        }
    }

    // The scheduler pins threads to specific cores. If those threads spawn a
    // new Inko process, those processes inherit the affinity and thus are
    // pinned to the same thread. This also result in Rust's
    // `available_parallelism()` function reporting 1, instead of e.g. 8 on a
    // system with 8 cores/threads.
    //
    // To fix this, we first reset the affinity so the default/current mask
    // allows use of all available cores/threads.
    reset_affinity();

    // We ignore all signals by default so they're routed to the signal handler
    // thread. This also takes care of ignoring SIGPIPE, which Rust normally
    // does for us when compiling an executable.
    signal_sched::block_all();

    // Configure the TLS provider. This must be done once before we start the
    // program.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to set up the default TLS cryptography provider");

    Box::into_raw(Box::new(Runtime::new(&*counts, args)))
}

#[no_mangle]
pub unsafe extern "system" fn inko_runtime_drop(runtime: *mut Runtime) {
    drop(Box::from_raw(runtime));
}

#[no_mangle]
pub unsafe extern "system" fn inko_runtime_start(
    runtime: *mut Runtime,
    class: ClassPointer,
    method: NativeAsyncMethod,
) {
    (*runtime).start(class, method);
    flush_stdout();
}

#[no_mangle]
pub unsafe extern "system" fn inko_runtime_state(
    runtime: *mut Runtime,
) -> *const State {
    (*runtime).state.as_ptr() as _
}

#[no_mangle]
pub unsafe extern "system" fn inko_runtime_stack_mask(
    runtime: *mut Runtime,
) -> u64 {
    let raw_size = (*runtime).state.config.stack_size;
    let total = total_stack_size(raw_size as _, page_size()) as u64;

    !(total - 1)
}

fn flush_stdout() {
    // STDOUT is buffered by default, and not flushing it upon exit may result
    // in parent processes not observing the output.
    let _ = stdout().lock().flush();
}

pub(crate) fn exit(status: i32) -> ! {
    flush_stdout();
    rust_exit(status);
}

/// An Inko runtime along with all its state.
#[repr(C)]
pub struct Runtime {
    state: RcState,
}

impl Runtime {
    /// Returns a new `Runtime` instance.
    ///
    /// This method sets up the runtime and allocates the core classes, but
    /// doesn't start any threads.
    fn new(counts: &MethodCounts, args: Vec<String>) -> Self {
        Self { state: State::new(Config::from_env(), counts, args) }
    }

    /// Starts the runtime using the given process and method as the entry
    /// point.
    ///
    /// This method blocks the current thread until the program terminates,
    /// though this thread itself doesn't run any processes (= it just
    /// waits/blocks until completion).
    fn start(&self, main_class: ClassPointer, main_method: NativeAsyncMethod) {
        let state = self.state.clone();

        thread::Builder::new()
            .name("timeout".to_string())
            .spawn(move || state.timeout_worker.run(&state))
            .unwrap();

        for id in 0..self.state.network_pollers.len() {
            let state = self.state.clone();

            thread::Builder::new()
                .name(format!("netpoll {}", id))
                .spawn(move || NetworkPollerWorker::new(id, state).run())
                .unwrap();
        }

        // Signal handling is very racy, meaning that if we notify the signal
        // handler to shut down it may not observe the signal correctly,
        // resulting in the program hanging. To prevent this from happening, we
        // simply don't wait for the signal handler thread to stop during
        // shutdown.
        {
            let state = self.state.clone();

            thread::Builder::new()
                .name("signals".to_string())
                .spawn(move || signal_sched::Worker::new(state).run())
                .unwrap();
        }

        let stack_size = self.state.config.stack_size as usize;
        let stack = Stack::new(stack_size, page_size());
        let main_proc = Process::main(main_class, main_method, stack);

        self.state.scheduler.run(&self.state, main_proc);
    }
}
