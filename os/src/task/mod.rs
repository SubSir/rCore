use crate::fs::open_file;
use crate::fs::OpenFlags;
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
use manager::add_task;
use processor::{schedule, take_curent_task};
use task::TaskControlBlock;

mod context;
pub mod manager;
mod pid;
pub mod processor;
mod switch;
mod task;
use crate::task::context::TaskContext;
use crate::task::task::TaskStatus;
use lazy_static::lazy_static;
use switch::__switch;

lazy_static! {
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
        let inode = open_file(0, "initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    });
}

pub fn add_initproc() {
    add_task(INITPROC.clone());
}

pub fn suspend_current_and_run_next() {
    let task = take_curent_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    add_task(task);
    schedule(task_cx_ptr);
}

pub fn exit_current_and_run_next(xstate: i32) {
    let task = take_curent_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    inner.exit_code = xstate;
    inner.task_status = TaskStatus::Zombie;

    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }

    inner.children.clear();
    inner.memory_set.recycle_data_pages();
    drop(inner);
    drop(task);
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}
