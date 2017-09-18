use std::sync::{Arc, Mutex, Condvar};
use std::time::Duration;
use std::sync::MutexGuard;

#[derive(Debug, PartialEq)]
pub enum ThreadStatus {
    Pending,
    Up,
    Aborting,
    Down,
}

#[derive(Clone)]
pub struct ThreadHandle {
    status: Arc<(Mutex<ThreadStatus>, Condvar)>,
}

/// Guarantee that one and only one thread is up
impl ThreadHandle {
    pub fn new() -> Self {
        ThreadHandle { status: Arc::new((Mutex::new(ThreadStatus::Down), Condvar::new())) }
    }

    fn wait<'a>(mut status: MutexGuard<'a, ThreadStatus>, cvar: &Condvar) -> MutexGuard<'a, ThreadStatus> {
        loop {
            let result = cvar.wait_timeout(status, Duration::from_millis(10))
                .unwrap();
            trace!("wait_for_thread_up: 10 ms passed: result: {:?} {:?}",
                   result,
                   *result.0);
            status = result.0;
            match *status {
                ThreadStatus::Up => break,
                ThreadStatus::Down => break,
                _ => continue
            }
        }
        status
    }

    pub fn wait_for_thread_up(&self) {
        let (ref lock, ref cvar) = *self.status.clone();
        let status = lock.lock().unwrap();
        trace!("wait_for_thread_up: enter");

        let _ = Self::wait(status, cvar);

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

    pub fn set_thread_down(&self) {
        let (ref lock, _) = *self.status;
        let mut status = lock.lock().unwrap();
        if let ThreadStatus::Aborting = *status {
            *status = ThreadStatus::Down
        }
        trace!("set thread down");
    }

    /// return false if init is already done
    pub fn thread_guard(&self) -> bool {
        let (ref lock, ref cvar) = *self.status;
        let mut status = lock.lock().unwrap();
        trace!("thread current status: {:?}", *status);
        match *status {
            ThreadStatus::Aborting => {
                trace!("aborting wait 0");
                let mut status = Self::wait(status, cvar);
                trace!("aborting wait 1");
                *status = ThreadStatus::Pending;
                true
            }
            ThreadStatus::Down => {
                *status = ThreadStatus::Pending;
                true
            }
            ThreadStatus::Pending => {
                trace!("pending wait 0");
                let _ = Self::wait(status, cvar);
                trace!("pending wait 1");
                false
            },
            ThreadStatus::Up => false,
        }
    }

    pub fn status(&self) -> MutexGuard<ThreadStatus> {
        let (ref lock, _) = *self.status;
        lock.lock().unwrap()
    }
}
