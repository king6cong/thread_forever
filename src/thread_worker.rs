use std::thread;
use std::time::Duration;
pub use errors::*;
use {Payload, RetryMethod};
use std::panic::{catch_unwind, AssertUnwindSafe};
use thread_handle::ThreadHandle;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::{RwLockReadGuard, RwLockWriteGuard};
use ::Cmd;

#[derive(Debug, Clone)]
pub enum Status {
    Down,
    Up,
}

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

        if !self.handle().thread_need_init() {
            info!("t:{} inited, no-op", name);
            return Ok(());
        }

        self.handle().set_status(Status::Up)?;

        let payload = payload.clone();
        let name_clone = name.clone();
        let _ = thread::Builder::new()
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

        self.handle().wait_for_thread_up();
        Ok(())
    }

    pub fn handle(&self) -> &Handle {
        self.payload.handle()
    }
}

pub trait RW<T> {
    fn read_lock(&self) -> RwLockReadGuard<T>;
    fn write_lock(&self) -> RwLockWriteGuard<T>;
}

impl<T> RW<T> for RwLock<T> {
    fn read_lock(&self) -> RwLockReadGuard<T> {
        self.read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    fn write_lock(&self) -> RwLockWriteGuard<T> {
        self.write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Clone)]
pub struct Handle {
    handle: ThreadHandle,
    cmd: Arc<RwLock<Cmd>>,
    status: Arc<RwLock<Status>>,
}

#[derive(Debug, Clone)]
pub enum SetCmdResult {
    Noop,
    Set,
}

impl Handle {
    pub fn new() -> Self {
        Handle {
            handle: ThreadHandle::new(),
            cmd: Arc::new(RwLock::new(Cmd::Noop)),
            status: Arc::new(RwLock::new(Status::Down)),
        }
    }
    pub fn wait_for_thread_up(&self) {
        self.handle.wait_for_thread_up()
    }

    pub fn notify_thread_up(&self) {
        self.handle.notify_thread_up()
    }

    pub fn set_thread_aborting(&self) {
        self.handle.set_thread_aborting()
    }

    pub fn thread_need_init(&self) -> bool {
        self.handle.thread_need_init()
    }

    pub fn status(&self) -> Status {
        self.status.read_lock().clone()
    }

    pub fn set_status(&self, status: Status) -> Result<()> {
        *self.status.write_lock() = status;
        Ok(())
    }

    pub fn set_cmd(&self, cmd: Cmd) -> Result<SetCmdResult> {
        match cmd {
            Cmd::Restart => {
                match self.status() {
                    Status::Up => *self.cmd.write_lock() = cmd,
                    Status::Down => {
                        info!("status is down and set_cmd is noop");
                        return Ok(SetCmdResult::Noop)
                    }
                }
            }
            Cmd::Noop => *self.cmd.write_lock() = cmd,
        }
        Ok(SetCmdResult::Set)
    }

    pub fn check_and_reset_cmd(&self) -> Cmd {
        trace!("CHECK_CMD: {:?}", self.cmd);
        let cmd = self.cmd.read_lock().clone();
        match cmd {
            Cmd::Restart => {
                *self.cmd.write_lock() = Cmd::Noop;
                Cmd::Restart
            }
            _ => Cmd::Noop
        }
    }
}

impl Default for Handle {
    fn default() -> Self { Handle::new() }
}


#[cfg(test)]
mod tests {
    extern crate env_logger;
    #[allow(unused_imports)]
    use super::*;

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
        handle: Handle,
    }

    impl Payload for Test {
        type Result = Result<()>;

        fn name(&self) -> String {
            "Test".to_string()
        }

        fn thread_func(&self) -> Result<()> {
            self.handle.notify_thread_up();

            loop {
                thread::sleep(Duration::from_millis(100));
                let cmd = self.handle.check_and_reset_cmd();
                info!("iter, cmd: {:?}", cmd);
                match cmd {
                    Cmd::Restart => return Ok(()),
                    _ => {},
                }
            }
        }

        fn handle(&self) -> &Handle {
            &self.handle
        }
    }

    impl Test {
        fn new() -> Self {
            Test { handle: Default::default() }
        }
        fn test_read(&self) {
            info!("test_read");
        }
    }

    fn test_thread_forever() {
        let _ = env_logger::init();
        restart();
        status();
        let _ = WORKER.spin_up();
        let _ = WORKER.spin_up();
        let _ = WORKER.spin_up();
        let _ = WORKER.spin_up();
        WORKER.payload.test_read();
        status();
        thread::sleep(Duration::from_millis(4000));
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

    fn restart() {
        let _ = WORKER.handle().set_cmd(Cmd::Restart);
    }

    fn status() {
        let status = WORKER.handle().status();
        info!("status: {:?}", status);
    }

    #[test]
    fn test_restart_1() {
        restart();
    }

    /// failure test
    #[derive(Clone)]
    pub struct TestFail {
        handle: Handle,
    }

    impl Payload for TestFail {
        type Result = Result<()>;

        fn name(&self) -> String {
            "TestFail".to_string()
        }

        fn thread_func(&self) -> Result<()> {
            self.handle.notify_thread_up();
            // panic!("panic test");
            // Ok(())
            Err(Error::from("error"))
        }

        fn handle(&self) -> &Handle {
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
            TestFail { handle: Default::default() }
        }
    }

    fn test_thread_forever_fail() {
        let _ = env_logger::init();
        let _ = WORKER_FAIL.spin_up();
        let _ = WORKER_FAIL.spin_up();
        let _ = WORKER_FAIL.spin_up();
        let _ = WORKER_FAIL.spin_up();
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
