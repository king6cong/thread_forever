use std::sync::{Arc, Mutex, Condvar};
use std::time::Duration;
use std::sync::MutexGuard;

#[derive(Debug, PartialEq)]
enum ThreadStatus {
    Uninitialized,
    Pending,
    Up,
    Aborting,
}

#[derive(Clone)]
pub struct ThreadHandle {
    status: Arc<(Mutex<ThreadStatus>, Condvar)>,
}

/// Guarantee that one and only one thread is up
impl ThreadHandle {
    pub fn new() -> Self {
        ThreadHandle { status: Arc::new((Mutex::new(ThreadStatus::Uninitialized), Condvar::new())) }
    }

    fn wait(mut status: MutexGuard<ThreadStatus>, cvar: &Condvar) {
        loop {
            let result = cvar.wait_timeout(status, Duration::from_millis(10))
                .unwrap();
            trace!("wait_for_thread_up: 10 ms passed: result: {:?} {:?}",
                   result,
                   *result.0);
            status = result.0;
            if let ThreadStatus::Up = *status {
                trace!("wait_for_thread_up: exit");
                break;
            }
        }
    }

    pub fn wait_for_thread_up(&self) {
        let (ref lock, ref cvar) = *self.status.clone();
        let status = lock.lock().unwrap();
        trace!("wait_for_thread_up: enter");

        Self::wait(status, cvar);

        trace!("wait_for_thread_up: exit");
    }

    pub fn notify_thread_up(&self) {
        let (ref lock, ref cvar) = *self.status;
        let mut status = lock.lock().unwrap();
        *status = ThreadStatus::Up;
        trace!("notify the condvar that thread is up.");
        cvar.notify_all();
    }

    pub fn set_thread_aborting(&self) {
        let (ref lock, _) = *self.status;
        let mut status = lock.lock().unwrap();
        *status = ThreadStatus::Aborting;
        trace!("set thread aborting");
    }

    /// return false if init is already done
    pub fn thread_need_init(&self) -> bool {
        let (ref lock, ref cvar) = *self.status;
        let mut status = lock.lock().unwrap();
        trace!("thread_need_init status: {:?}", *status);
        match *status {
            ThreadStatus::Aborting => {
                *status = ThreadStatus::Pending;
                true
            }
            ThreadStatus::Uninitialized => {
                *status = ThreadStatus::Pending;
                true
            }
            ThreadStatus::Pending => {
                trace!("pending wait 0");

                Self::wait(status, cvar);

                trace!("pending wait 1");
                false
            }
            ThreadStatus::Up => false,
        }
    }
}
