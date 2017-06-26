use std::thread;
use std::time::Duration;
pub use errors::*;
use {Payload, RetryMethod};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub struct ThreadWorker<T> {
    pub payload: T,
    name: String,
}

const DEFAULT_SLEEP: u64 = 1000;

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

    pub fn spin_up(&self) -> Result<()> {
        let payload = self.payload.clone();
        let name = self.name.clone();

        if !self.payload.handle().thread_need_init() {
            info!("t:{} inited, no-op", name);
            return Ok(());
        }

        let payload = payload.clone();
        let name_clone = name.clone();
        let thread_result = thread::Builder::new()
            .name(format!("t:{}", name))
            .spawn(move || -> Result<()> {
                loop {
                    let result = catch_unwind(AssertUnwindSafe(|| payload.thread_func()));
                    let retry_method = catch_unwind(AssertUnwindSafe(|| payload.on_exit(&result)));

                    info!("t:{} <thread_func> exited: <{:?}> retry_method: <{:?}>",
                          name_clone,
                          result,
                          retry_method);

                    match retry_method {
                        Ok(RetryMethod::Retry { after }) => {
                            info!("t:{} retry in: {:?}", name_clone, after);
                            thread::sleep(after);
                        }
                        Ok(RetryMethod::Abort) => {
                            error!("t:{} aborted", name_clone);
                            break;
                        }
                        Err(e) => {
                            error!("t:{}: <on_exit> panicked: {:?}", name_clone, e);
                            thread::sleep(Duration::from_millis(DEFAULT_SLEEP));
                        }
                    }
                }

                Ok(())
            })?;

        self.payload.handle().wait_for_thread_up();
        Ok(())
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
            // Ok(())
            Err(Error::from("error"))
        }

        fn handle(&self) -> &ThreadHandle {
            &self.handle
        }

        fn on_exit(&self, result: &thread::Result<Self::Result>) -> RetryMethod {
            trace!("on_exit: {:?}", result);
            // panic!("panic test");
            // let retry = RetryMethod::Retry { after: Duration::from_millis(1000) };
            let retry = RetryMethod::Abort;
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
