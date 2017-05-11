use std::thread;
use thread_handle::ThreadHandle;
pub use errors::*;
use Payload;

pub struct ThreadWorker<T> {
    pub payload: T,
    handle: ThreadHandle,
}

impl<T> ThreadWorker<T>
    where T: Payload + Clone + Send + 'static
{
    pub fn new(payload: T) -> Self {
        ThreadWorker {
            payload: payload,
            handle: ThreadHandle::new(),
        }
    }

    pub fn spin_up(&self) {
        let handle = self.handle.clone();
        let payload = self.payload.clone();
        let name = self.payload.name();

        if !self.handle.thread_init() {
            info!("{} worker is already initialized and we return directly!",
                  name);
            return;
        }

        let _ = thread::Builder::new()
            .name(format!("t:{}_watchdog", name))
            .spawn(move || -> Result<()> {
                debug!("{}_watchdog started!", name);
                while {
                          let handle = handle.clone();
                          let payload = payload.clone();
                          thread::Builder::new()
                              .name(format!("t:{}", name))
                              .spawn(move || -> Result<()> {

                                         handle.notify_thread_up();

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

        self.handle.wait_for_thread_up();
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_thread_forever() {

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
                    println!("one loop iter");
                }
            }
        }

        impl Test {
            fn new() -> Self {
                Test {}
            }
        }

        WORKER.spin_up();
        WORKER.spin_up();
        WORKER.spin_up();
        thread::sleep(Duration::from_millis(600));
    }
}
