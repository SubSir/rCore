use crate::fs::open_file;
use crate::fs::OpenFlags;
use crate::mm::translated_ref;
use crate::mm::translated_refmut;
use crate::mm::translated_str;
use crate::task::exit_current_and_run_next;
use crate::task::processor::current_process;
use crate::task::processor::current_task;
use crate::task::processor::current_user_token;
use crate::task::suspend_current_and_run_next;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::panic;
pub fn sys_exit(xstate: i32) -> ! {
    exit_current_and_run_next(xstate);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    // println!("[kernel] Application yields!");
    suspend_current_and_run_next();
    // println!("[kernel] Application resumes!");
    0
}

pub fn sys_get_time() -> isize {
    use crate::timer::get_time_ms;
    get_time_ms() as isize
}

pub fn sys_getpid() -> isize {
    current_task().unwrap().process.upgrade().unwrap().getpid() as isize
}

pub fn sys_fork() -> isize {
    let current_process = current_process();
    let new_process = current_process.fork();
    let new_pid = new_process.getpid();
    let new_process_inner = new_process.inner_exclusive_access();
    let task = new_process_inner.tasks[0].as_ref().unwrap();
    let trap_cx = task.inner_exclusive_access().get_trap_cx();
    trap_cx.x[10] = 0;
    new_pid as isize
}
pub fn sys_exec(id: usize, path: *const u8, mut args: *const usize) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    let mut args_vec: Vec<String> = Vec::new();
    loop {
        let arg_str_ptr = *translated_ref(token, args);
        if arg_str_ptr == 0 {
            break;
        } else {
            args_vec.push(translated_str(token, arg_str_ptr as *const u8));
            unsafe {
                args = args.add(1);
            }
        }
    }
    if let Some(data) = open_file(id, path.as_str(), OpenFlags::RDONLY) {
        let all_data = data.read_all();
        let process = current_process();
        let argc = args_vec.len();
        process.exec(all_data.as_slice(), args_vec);
        argc as isize
    } else {
        -1
    }
}

pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if inner
        .children
        .iter()
        .find(|p| pid == -1 || pid as usize == p.getpid())
        .is_none()
    {
        return -1;
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        p.inner_exclusive_access().is_zombie && (pid == -1 || pid as usize == p.getpid())
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        let exit_code = child.inner_exclusive_access().exit_code;
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
}

pub fn sys_kill() -> isize {
    exit_current_and_run_next(-4);
    0
}
