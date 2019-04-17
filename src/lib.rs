#[cfg(test)]
extern crate rand;
#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;
extern crate parking_lot;

use std::fmt;
use std::thread;
use std::time::Duration;
mod thread_handle;
mod thread_worker;
pub use thread_worker::{ThreadWorker, Handle, SetCmdResult};
pub use thread_handle::ThreadHandle;

#[derive(Debug)]
pub enum RetryMethod {
    Retry { after: Duration },
    Abort,
}

#[derive(Debug, Clone)]
pub enum Cmd {
    Restart,
    Noop,
}

pub trait Payload {
    type Result: fmt::Debug;

    fn name(&self) -> String;

    fn thread_func(&self) -> Self::Result;

    fn handle(&self) -> &Handle;

    /// on_exit will be called with catch_unwind result of thread_func
    fn on_exit(&self, result: &thread::Result<Self::Result>) -> RetryMethod {
        let retry = match *result {
            Ok(_) => RetryMethod::Retry { after: Duration::from_millis(2000) },
            Err(_) => RetryMethod::Retry { after: Duration::from_millis(0) },
        };
        trace!("on_exit: {:?} retry: {:?}", result, retry);
        retry
    }

    #[cfg(test)]
    fn send_enter(&self, id: &str);
    #[cfg(test)]
    fn send_exit(&self, id: &str, is_spin_up: bool);
}
