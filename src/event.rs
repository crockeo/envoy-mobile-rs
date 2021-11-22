use std::sync::{Condvar, Mutex};

pub struct Event {
    occurred: Mutex<bool>,
    condvar: Condvar,
}

impl Event {
    pub fn new() -> Self {
	Self {
	    occurred: Mutex::new(false),
	    condvar: Condvar::new(),
	}
    }

    pub fn set(&self) {
	let mut guard = self.occurred.lock().unwrap();
	if !*guard {
	    (*guard) = true;
	    self.condvar.notify_all();
	}
    }

    pub fn is_set(&self) -> bool {
	let guard = self.occurred.lock().unwrap();
	*guard
    }

    pub fn wait(&self) {
	let guard = self.occurred.lock().unwrap();
	let _ = self.condvar.wait_while(guard, |occurred| !(*occurred));
    }
}
