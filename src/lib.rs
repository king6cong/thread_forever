#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;

use std::fmt;
use std::thread;
use std::time::Duration;
mod thread_handle;
mod thread_worker;
mod errors;
pub use errors::*;
pub use thread_worker::ThreadWorker;
pub use thread_handle::ThreadHandle;

#[derive(Debug)]
pub enum RetryMethod {
    Retry { after: Duration },
    Abort,
}

pub trait Payload {
    type Result: fmt::Debug;
    fn name(&self) -> String;
    fn thread_func(&self) -> Self::Result;
    fn handle(&self) -> &ThreadHandle;
    /// on_exit will be called with catch_unwind result of thread_func
    fn on_exit(&self, result: &thread::Result<Self::Result>) -> RetryMethod {
        let retry = match *result {
            Ok(_) => RetryMethod::Retry { after: Duration::from_millis(2000) },
            Err(_) => RetryMethod::Retry { after: Duration::from_millis(0) },
        };
        trace!("on_exit: {:?} retry: {:?}", result, retry);
        retry
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
