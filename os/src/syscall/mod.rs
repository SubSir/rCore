const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;

use crate::syscall::fs::sys_write;
use crate::syscall::process::sys_exit;
mod fs;
mod process;

pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    match syscall_id {
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
