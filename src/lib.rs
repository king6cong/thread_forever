#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;

mod thread_handle;
mod thread_worker;
mod errors;
pub use errors::*;
pub use thread_worker::ThreadWorker;
pub use thread_handle::ThreadHandle;

pub trait Payload {
    type Result;
    fn name(&self) -> String;
    fn thread_func(&self) -> Self::Result;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
