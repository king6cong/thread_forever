use std::thread;
use std::time::Duration;
pub use errors::*;
use {Payload, RetryMethod};
use std::panic::{catch_unwind, AssertUnwindSafe};
use thread_handle::{ThreadHandle, ThreadStatus};
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::{RwLockReadGuard, RwLockWriteGuard};
use ::Cmd;

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

        #[cfg(test)]
        let unique_id = {
            use rand::{Rng, thread_rng};
            thread_rng().gen_ascii_chars().take(10).collect::<String>()
        };
        #[cfg(test)]
        self.payload.send_enter(&unique_id);
        if !self.handle().thread_guard() {
            info!("t:{} inited, no-op", name);
            #[cfg(test)]
            self.payload.send_exit(&unique_id, false);
            return Ok(());
        }
        #[cfg(test)]
        self.payload.send_exit(&unique_id, true);


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
                payload.handle().set_thread_down();

                Ok(())
            })?;

        self.handle().wait_for_thread_up();
        Ok(())
    }

    pub fn handle(&self) -> &Handle {
        self.payload.handle()
    }

    pub fn restart(&self) -> Result<()> {
        let name = self.name.clone();
        if self.handle().is_running() {
            self.handle().set_cmd(Cmd::Restart)?;
            trace!("thread {:?} set cmd restart success", name);
        } else {
            self.spin_up()?;
            trace!("spin up thread {:?} success", name);
        }
        Ok(())
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

    pub fn set_thread_down(&self) {
        self.handle.set_thread_down()
    }

    pub fn thread_guard(&self) -> bool {
        self.handle.thread_guard()
    }

    pub fn is_running(&self) -> bool {
        match *self.handle.status() {
            ThreadStatus::Up => true,
            ThreadStatus::Pending => true,
            ThreadStatus::Aborting => false,
            ThreadStatus::Down => false
        }
    }

    fn set_cmd(&self, cmd: Cmd) -> Result<SetCmdResult> {
        match cmd {
            Cmd::Restart => {
                match self.is_running() {
                    true => *self.cmd.write_lock() = cmd,
                    false => {
                        info!("current thread need spin up, set cmd to noop");
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
    use std::time::Instant;
    use std::sync::mpsc::{Sender, Receiver, channel};
    use std::sync::{Arc, Mutex, MutexGuard};

    #[macro_use]
    mod macros {
        macro_rules! run_worker {
            ($name:ident, $action:ident, $times:expr) => {
                thread::spawn(|| {
                    for _ in 0..$times {
                        let _ = $name.$action();
                    }
                });
            };
        }

        macro_rules! new_worker {
            ($name:ident, $is_success:expr) => {
                lazy_static! {
                    pub static ref $name: ThreadWorker<Test> = {
                        let payload = Test::new($is_success);
                        let worker = ThreadWorker::new(payload);
                        worker
                    };
                }
            };
        }

        macro_rules! increase_and_check {
            ($var:ident, $max_value:expr) => {
                $var = $var + 1;
                if $var == $max_value {
                    break;
                }
            };
        }
    }

    #[derive(Debug)]
    pub enum Signal {
        Enter,
        Exit(bool, Instant),
        Restart,
        Abort
    }

    pub struct Data {
        id: String,
        signal: Signal,
    }

    pub fn send_enter(tx: &MutexGuard<Sender<Data>>, id: &str) {
        let _ = tx.send(Data {
            id: id.to_string(),
            signal: Signal::Enter
        });
    }

    pub fn send_exit(tx: &MutexGuard<Sender<Data>>, is_spin_up: bool, id: &str) {
        let _ = tx.send(Data{
            id: id.to_string(),
            signal: Signal::Exit(is_spin_up, Instant::now())
        });
    }

    #[derive(Clone)]
    pub struct Test {
        handle: Handle,
        tx: Arc<Mutex<Sender<Data>>>,
        rx: Arc<Mutex<Receiver<Data>>>,
        is_success: bool
    }

    impl Payload for Test {
        type Result = Result<()>;

        fn name(&self) -> String {
            String::from("Test")
        }

        fn thread_func(&self) -> Result<()> {
            self.handle.notify_thread_up();

            loop {
                thread::sleep(Duration::from_millis(100));
                let cmd = self.handle.check_and_reset_cmd();
                info!("iter, cmd: {:?}", cmd);
                if self.is_success {
                    match cmd {
                        Cmd::Restart => {
                            let _ = self.tx.lock().unwrap().send(Data {
                                id: String::from("restart"),
                                signal: Signal::Restart
                            });
                            return Ok(())
                        },
                        _ => {},
                    }
                } else {
                    return Err(Error::from("error"))
                }
            }
        }

        fn on_exit(&self, result: &thread::Result<Self::Result>) -> RetryMethod {
            trace!("on_exit: {:?}", result);
            match *result {
                Ok(Err(_)) => {
                    self.handle().set_thread_aborting();
                    let _ = self.tx.lock().unwrap().send(Data {
                        id: String::from("abort"),
                        signal: Signal::Abort
                    });
                    RetryMethod::Abort
                }
                _ => {
                    RetryMethod::Retry {
                        after: Duration::from_millis(200)
                    }
                }
            }
        }

        fn handle(&self) -> &Handle {
            &self.handle
        }
        fn send_enter(&self, id: &str) {
            send_enter(&self.tx.lock().unwrap(), id);
        }

        fn send_exit(&self, id: &str, is_spin_up: bool) {
            send_exit(&self.tx.lock().unwrap(), is_spin_up, id);
        }
    }

    impl Test {
        fn new(is_success: bool) -> Self {
            let (tx, rx) = channel();
            Test {
                handle: Default::default(),
                tx: Arc::new(Mutex::new(tx)),
                rx: Arc::new(Mutex::new(rx)),
                is_success: is_success
            }
        }

        fn get_rx(&self) -> MutexGuard<Receiver<Data>> {
            self.rx.lock().unwrap()
        }
    }

    new_worker!(WORKER_SUCCESS, true);

    #[test]
    /// Test the thread_worker will be spin up only once
    fn test_spin_up_once() {
        let _ = env_logger::init();
        run_worker!(WORKER_SUCCESS, spin_up, 2);
        run_worker!(WORKER_SUCCESS, spin_up, 2);
        run_worker!(WORKER_SUCCESS, spin_up, 2);

        let mut spin_up_num = 0;
        let mut skip_num = 0;
        let rx = &*WORKER_SUCCESS.payload.get_rx();

        let mut counter = 0;
        for data in rx.iter() {
            match data.signal {
                Signal::Exit(true, _) => spin_up_num = spin_up_num + 1,
                Signal::Exit(false, _) => skip_num = skip_num + 1,
                _ => { continue }
            }
            increase_and_check!(counter, 6);
        }
        assert_eq!(spin_up_num, 1);
        assert_eq!(skip_num, 5);
    }

    new_worker!(WORKER_WAIT, true);

    #[test]
    /// Test first call spin up worker will exit spin up first
    fn test_spin_up_wait() {
        let _ = env_logger::init();
        run_worker!(WORKER_WAIT, spin_up, 1);
        run_worker!(WORKER_WAIT, spin_up, 1);

        let mut first_in = None;
        let mut second_in = None;
        let mut first_out_time = None;
        let mut second_out_time = None;
        let rx = &*WORKER_WAIT.payload.get_rx();
        let mut counter = 0;
        for data in rx.iter() {
            let Data { id, signal } = data;
            match signal {
                Signal::Enter => {
                    match first_in {
                        Some(_) => second_in = Some(id),
                        None => first_in = Some(id),
                    }
                }
                 Signal::Exit(is_spin_up, time) => {
                    let name = Some(id);
                    if name == first_in {
                        first_out_time = Some(time);
                        assert!(is_spin_up);
                    } else if name == second_in {
                        second_out_time = Some(time);
                        assert!(!is_spin_up);
                    } else {
                        panic!("This is a Bug in Test");
                    }
                }
                _ => { continue }
            }
            increase_and_check!(counter, 4);
        }
        assert!(first_out_time.unwrap().lt(&second_out_time.unwrap()));
    }

    new_worker!(WORKER_RESTART, true);

    #[test]
    /// Test the restart will spin up current worker if worker is down
    /// And will restart current worker if worker is up
    fn test_restart_success() {
        let _ = env_logger::init();
        run_worker!(WORKER_RESTART, restart, 1);

        let mut first_time = true;
        let rx = &*WORKER_RESTART.payload.get_rx();
        let mut counter = 0;
        for data in rx.iter() {
            match data.signal {
                Signal::Exit(true, _) => {
                    assert!(first_time);
                    first_time = false;
                    run_worker!(WORKER_RESTART, restart, 1);
                }
                Signal::Restart => {
                    assert!(!first_time);
                }
                _ => { continue }
            }
            increase_and_check!(counter, 2);
        }
    }

    new_worker!(WORKER_FAIL, false);

    #[test]
    /// Test aborted work can be spin up
    fn test_fail_worker_spin_up() {
        let _ = env_logger::init();
        run_worker!(WORKER_FAIL, spin_up, 1);

        let mut spin_up_time = 0;
        let rx = &*WORKER_FAIL.payload.get_rx();
        let mut counter = 0;
        for data in rx.iter() {
            match data.signal {
                Signal::Exit(true, _)=> {
                    spin_up_time = spin_up_time + 1;
                }
                Signal::Abort => {
                    run_worker!(WORKER_FAIL, spin_up, 1);
                }
                _ => { continue }
            }
            increase_and_check!(counter, 4);
        }
        assert_eq!(spin_up_time, 2);
    }

    new_worker!(WORKER_FAIL2, false);

    #[test]
    /// Test restart will spin up aborted worker
    fn test_fail_worker_restart() {
        let _ = env_logger::init();
        run_worker!(WORKER_FAIL2, spin_up, 1);

        let mut spin_up_num = 0;
        let rx = &*WORKER_FAIL2.payload.get_rx();
        let mut counter = 0;
        for data in rx.iter() {
            match data.signal {
                Signal::Exit(true, _) => {
                    spin_up_num = spin_up_num + 1;
                }
                Signal::Abort => {
                    run_worker!(WORKER_FAIL2, restart, 1);
                }
                _ => { continue }
            }
            increase_and_check!(counter, 4);
        }
        assert_eq!(spin_up_num, 2);
    }
}
