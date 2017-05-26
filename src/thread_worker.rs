use std::thread;
use std::time::Duration;
pub use errors::*;
use {Payload, RetryMethod};

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
                    let thread_result = thread::Builder::new()
                        .name(format!("t:{}", name))
                        .spawn(move || -> Result<RetryMethod> {
                            let result = payload.thread_func();
                            let retry_method = payload.on_error(&result);
                            error!("thread_func of {} exited: {:?} retry_method: {:?}",
                                   name_clone,
                                   result,
                                   retry_method);

                            Ok(retry_method)
                        })?
                        .join();
                    warn!("{} worker respawn, thread_result: {:?}",
                          name,
                          thread_result);

                    match thread_result {
                        Ok(retry_method) => {
                            match retry_method {
                                Ok(RetryMethod::Retry { after }) => {
                                    thread::sleep(after);
                                    info!("retry_method sleep: {:?}", after);
                                }
                                Ok(RetryMethod::Abort) => {
                                    info!("retry_method break");
                                    break;
                                }
                                Err(e) => {
                                    error!("unexpected error: {:?}", e);
                                }
                            }
                        }
                        Err(error) => {
                            info!("thread error: {:?}, should be caused by a panic", error);
                            thread::sleep(Duration::from_millis(1000));
                        }
                    }

                }
                Ok(())
            });
        self.payload.handle().wait_for_thread_up();
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

        pub static ref WORKER_FAIL: ThreadWorker<TestFail> = {
            let payload = TestFail::new();
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


    /// failure test
    #[derive(Clone)]
    pub struct TestFail {
        handle: ThreadHandle,
    }

    impl Payload for TestFail {
        type Result = Result<()>;

        fn name(&self) -> String {
            "thread_forever_test".to_string()
        }

        fn thread_func(&self) -> Result<()> {
            self.handle.notify_thread_up();
            // panic!("panic test");
            return Err(Error::from("error test"));
        }

        fn handle(&self) -> &ThreadHandle {
            &self.handle
        }

        fn on_error(&self, result: &Self::Result) -> RetryMethod {
            info!("on_error: {:?}", result);
            // panic!("panic test");
            // let retry = RetryMethod::Retry { after: Duration::from_millis(1000) };
            let retry = RetryMethod::Abort;
            info!("retry: {:?}", retry);
            retry
        }
    }

    impl TestFail {
        fn new() -> Self {
            TestFail { handle: ThreadHandle::new() }
        }
    }

    fn test_thread_forever_fail() {
        let _ = env_logger::init();
        WORKER_FAIL.spin_up();
        WORKER_FAIL.spin_up();
        WORKER_FAIL.spin_up();
        WORKER_FAIL.spin_up();
        thread::sleep(Duration::from_millis(4000));
    }

    #[test]
    fn test_fail_1() {
        test_thread_forever_fail();
    }

    #[test]
    fn test_fail_2() {
        test_thread_forever_fail();
    }

    #[test]
    fn test_fail_3() {
        test_thread_forever_fail();
    }

}
