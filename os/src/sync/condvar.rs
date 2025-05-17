use super::UPSafeCell;
use crate::task::task::TaskControlBlock;
use alloc::{collections::vec_deque::VecDeque, sync::Arc};

pub struct Condvar {
    pub inner: UPSafeCell<CondvarInner>,
}

pub struct CondvarInner {
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}
