use crate::fs::open_file;
use crate::fs::OpenFlags;
use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use crate::task::processor::schedule2;
use alloc::sync::Arc;
use alloc::vec::Vec;
use id::TaskUserRes;
use id::IDLE_PID;
use manager::add_task;
use manager::count;
use manager::remove_from_pid2process;
use manager::TASK_MANAGER;
use processor::{schedule, take_curent_task};
use task::TaskControlBlock;

mod context;
mod id;
pub mod manager;
mod process;
use crate::task::process::ProcessControlBlock;
pub mod processor;
mod switch;
pub mod task;
use crate::task::context::TaskContext;
use crate::task::task::TaskStatus;
use lazy_static::lazy_static;
use switch::__switch;

lazy_static! {
    pub static ref INITPROC: Arc<ProcessControlBlock> = {
        let inode = open_file(0, "initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        ProcessControlBlock::new(v.as_slice())
    };
}

pub fn add_initproc() {
    let _initproc = INITPROC.clone();
}
pub fn suspend_current_and_run_next() {
    // println!("Suspend current and run next");
    let task = take_curent_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    add_task(task);
    schedule(task_cx_ptr);
}

pub fn suspend_current_and_run_next2() {
    // println!("Suspend current and run next");
    let task = take_curent_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    add_task(task);
    schedule2(task_cx_ptr);
}
pub fn block_current_and_run_next() {
    let task = take_curent_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Blocked;
    drop(task_inner);
    schedule(task_cx_ptr);
}

pub fn exit_current_and_run_next(xstate: i32) {
    let task = take_curent_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let process = task.process.upgrade().unwrap();
    let tid = task_inner.res.as_ref().unwrap().tid;
    task_inner.exit_code = Some(xstate);
    task_inner.res = None;
    drop(task_inner);
    drop(task);
    if tid == 0 {
        let pid = process.getpid();
        if pid == IDLE_PID {
            println!("[kernel] idle process exit with xstate {}", xstate);
            if xstate != 0 {
                shutdown(true);
            } else {
                shutdown(false);
            }
        }
        remove_from_pid2process(pid);
        let mut process_inner = process.inner_exclusive_access();
        process_inner.is_zombie = true;
        process_inner.exit_code = xstate;

        {
            let mut initproc_inner = INITPROC.inner_exclusive_access();
            for child in process_inner.children.iter() {
                child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
                initproc_inner.children.push(child.clone());
            }
        }

        let mut recycle_res = Vec::<TaskUserRes>::new();
        for task in process_inner.tasks.iter().filter(|t| t.is_some()) {
            let task = task.as_ref().unwrap();
            remove_task(Arc::clone(task));
            let mut task_inner = task.inner_exclusive_access();
            if let Some(res) = task_inner.res.take() {
                recycle_res.push(res);
            }
        }
        drop(process_inner);
        recycle_res.clear();

        let mut process_inner = process.inner_exclusive_access();
        process_inner.children.clear();
        process_inner.memory_set.recycle_data_pages();
        process_inner.fd_table.clear();
        while process_inner.tasks.len() > 1 {
            process_inner.tasks.pop();
        }
    }
    drop(process);
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

pub fn remove_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().remove(task);
}
