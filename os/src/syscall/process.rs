use core::panic;

use crate::task::exit_current_and_run_next;
use crate::task::suspend_crttent_and_run_next;

pub fn sys_exit(xstate: i32) -> ! {
    println!("[kernel] Application exited with code {}", xstate);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    suspend_crttent_and_run_next();
    0
}
