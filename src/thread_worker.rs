use std::thread;
// use std;
use std::time::Duration;
// use std::panic;
// use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, Arc};
// use thread_handle::ThreadHandle;
pub use errors::*;
use Payload;

pub struct ThreadWorker<T> {
    pub payload: T,
    name: String,
}

impl<T> ThreadWorker<T>
    where T: Payload + Clone + Send + Sync + 'static
{
    pub fn new(payload: T) -> Self {
        let name = payload.name();
        ThreadWorker {
            payload: payload,
            name: name,
        }
    }

    pub fn spin_up(&self) {
        let payload = self.payload.clone();
        let name = self.name.clone();
        trace!("0");
        if !self.payload.handle().thread_need_init() {
            info!("{} worker already initialized, return directly!", name);
            return;
        }
        trace!("1");

        let _ = thread::Builder::new()
            .name(format!("t:{}_watchdog", name))
            .spawn(move || -> Result<()> {
                trace!("{}_watchdog spawn", name);

                loop {
                    let payload = payload.clone();
                    let name_clone = name.clone();
                    let _ = thread::Builder::new()
                        .name(format!("t:{}", name))
                        .spawn(move || -> Result<()> {
                            trace!("2");
                            // handle.notify_thread_up();
                            trace!("3");

                            let result = payload.thread_func();
                            error!("thread_func of {} exited: {:?}", name_clone, result);

                            Ok(())
                        })?
                        .join();
                    thread::sleep(Duration::from_millis(600));
                    warn!("{} worker respawn!", name);
                }
            });
        trace!("4");
        self.payload.handle().wait_for_thread_up();
        trace!("5");
    }
}

#[cfg(test)]
mod tests {
    extern crate env_logger;
    #[allow(unused_imports)]
    use super::*;
    use thread_handle::ThreadHandle;

    lazy_static! {
        pub static ref WORKER: ThreadWorker<Test> = {
            let payload = Test::new();
            let worker = ThreadWorker::new(payload);
            worker
        };
    }

    #[derive(Clone)]
    pub struct Test {
        handle: ThreadHandle,
    }

    impl Payload for Test {
        type Result = Result<()>;

        fn name(&self) -> String {
            "thread_forever_test".to_string()
        }

        fn thread_func(&self) -> Result<()> {
            self.handle.notify_thread_up();
            // panic!("panic test");
            // return Err(Error::from("error test"));

            loop {
                thread::sleep(Duration::from_millis(100));
                info!("one loop iter");
            }
        }

        fn handle(&self) -> &ThreadHandle {
            &self.handle
        }
    }

    impl Test {
        fn new() -> Self {
            Test { handle: ThreadHandle::new() }
        }
        fn test_read(&self) {
            info!("test_read");
        }
    }

    fn test_thread_forever() {
        let _ = env_logger::init();
        WORKER.spin_up();
        WORKER.spin_up();
        WORKER.spin_up();
        WORKER.spin_up();
        WORKER.payload.test_read();
        thread::sleep(Duration::from_millis(2000));
    }

    #[test]
    fn test_1() {
        test_thread_forever();
    }

    #[test]
    fn test_2() {
        test_thread_forever();
    }

    #[test]
    fn test_3() {
        test_thread_forever();
    }
}
