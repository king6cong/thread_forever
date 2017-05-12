use std::thread;
// use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, Arc};
use thread_handle::ThreadHandle;
pub use errors::*;
use Payload;

pub struct ThreadWorker<T> {
    // payload: Arc<RwLock<T>>,
    pub payload: T,
    handle: ThreadHandle,
    name: String,
}

impl<T> ThreadWorker<T>
    where T: Payload + Clone + Send + Sync + 'static
{
    pub fn new(payload: T) -> Self {
        let name = payload.name();
        ThreadWorker {
            // payload: Arc::new(RwLock::new(payload)),
            payload: payload,
            handle: ThreadHandle::new(),
            name: name,
        }
    }

    pub fn spin_up(&self) {
        let handle = self.handle.clone();
        let payload = self.payload.clone();
        let name = self.name.clone();
        trace!("0");
        if !self.handle.thread_need_init() {
            info!("{} worker already initialized, return directly!", name);
            return;
        }
        trace!("1");

        let _ = thread::Builder::new()
            .name(format!("t:{}_watchdog", name))
            .spawn(move || -> Result<()> {
                trace!("{}_watchdog spawn", name);
                while {
                          let handle = handle.clone();
                          let payload = payload.clone();
                          thread::Builder::new()
                              .name(format!("t:{}", name))
                              .spawn(move || -> Result<()> {
                        trace!("2");
                        handle.notify_thread_up();
                        trace!("3");

                        let _ = payload.thread_func();


                        Ok(())
                    })?
                              .join()
                              .is_err()
                      } {
                    warn!("{} worker respawn!", name);
                }
                Ok(())
            });
        trace!("4");
        self.handle.wait_for_thread_up();
        trace!("5");
    }
}

#[cfg(test)]
mod tests {
    extern crate env_logger;
    #[allow(unused_imports)]
    use super::*;
    use std::time::Duration;

    lazy_static! {
        pub static ref WORKER: ThreadWorker<Test> = {
            let payload = Test::new();
            let worker = ThreadWorker::new(payload);
            worker
        };
    }

    #[derive(Clone)]
    pub struct Test {}

    impl Payload for Test {
        type Result = Result<()>;

        fn name(&self) -> String {
            "thread_forever_test".to_string()
        }

        fn thread_func(&self) -> Result<()> {
            loop {
                thread::sleep(Duration::from_millis(200));
                info!("one loop iter");
            }
        }
    }

    impl Test {
        fn new() -> Self {
            Test {}
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
        thread::sleep(Duration::from_millis(600));
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
