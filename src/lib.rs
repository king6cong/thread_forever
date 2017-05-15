#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;

use std::fmt;
mod thread_handle;
mod thread_worker;
mod errors;
pub use errors::*;
pub use thread_worker::ThreadWorker;
pub use thread_handle::ThreadHandle;

pub trait Payload {
    type Result: fmt::Debug;
    fn name(&self) -> String;
    fn thread_func(&self) -> Self::Result;
    // fn thread_func(&self) -> std::result::Result;
    fn handle(&self) -> &ThreadHandle;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
