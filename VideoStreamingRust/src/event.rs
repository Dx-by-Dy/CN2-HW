use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// Аналог threading.Event из Python.
#[derive(Default)]
pub struct Event {
    flag: Mutex<bool>,
    cv: Condvar,
}

impl Event {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self) {
        *self.flag.lock().unwrap() = true;
        self.cv.notify_all();
    }

    pub fn is_set(&self) -> bool {
        *self.flag.lock().unwrap()
    }

    /// Ждем установки флага не дольше `timeout`. Возвращаем значение флага.
    pub fn wait(&self, timeout: Duration) -> bool {
        let flag = self.flag.lock().unwrap();
        let (flag, _) = self.cv.wait_timeout_while(flag, timeout, |set| !*set).unwrap();
        *flag
    }
}
