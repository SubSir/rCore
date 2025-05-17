use alloc::sync::Arc;

use crate::task::{
    manager::wakeup_task, processor::current_task, suspend_current_and_run_next,
    task::TaskControlBlock,
};

use super::UPSafeCell;
use crate::task::block_current_and_run_next;
use alloc::collections::VecDeque;

pub trait Mutex: Sync + Send {
    fn lock(&self);
    fn unlock(&self);
}

pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    pub fn new() -> Self {
        MutexSpin {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    fn lock(&self) {
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                break;
            }
        }
    }

    fn unlock(&self) {
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

impl MutexBlocking {
    pub fn new() -> Self {
        MutexBlocking {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Mutex for MutexBlocking {
    fn lock(&self) {
        let mut inner = self.inner.exclusive_access();
        if inner.locked {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        } else {
            inner.locked = true;
        }
    }

    fn unlock(&self) {
        let mut inner = self.inner.exclusive_access();
        assert!(inner.locked);
        if let Some(waking_task) = inner.wait_queue.pop_front() {
            wakeup_task(waking_task);
        } else {
            inner.locked = false;
        }
    }
}
