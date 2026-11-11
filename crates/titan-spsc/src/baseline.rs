//! `Mutex<VecDeque>` channel used as the naive reference point.

use crate::{Rx, Tx};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct MutexChannel {
    q: Arc<Mutex<VecDeque<u64>>>,
}

pub fn channel(capacity: usize) -> (MutexChannel, MutexChannel) {
    let q = Arc::new(Mutex::new(VecDeque::with_capacity(capacity)));
    (MutexChannel { q: q.clone() }, MutexChannel { q })
}

impl Tx for MutexChannel {
    fn send(&mut self, v: u64) {
        self.q.lock().unwrap().push_back(v);
    }
}

impl Rx for MutexChannel {
    fn recv(&mut self) -> u64 {
        loop {
            if let Some(v) = self.q.lock().unwrap().pop_front() {
                return v;
            }
            std::hint::spin_loop();
        }
    }
}
