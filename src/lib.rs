#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;

use std::fmt;
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
    fn on_error(&self, result: &Self::Result) -> RetryMethod {
        let retry = RetryMethod::Retry { after: Duration::from_millis(2000) };
        trace!("on_error: {:?} retry: {:?}", result, retry);
        retry
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
