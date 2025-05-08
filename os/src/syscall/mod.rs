const SYSCALL_DUP: usize = 24;
const SYSCALL_OPEN: usize = 56;
const SYSCALL_CLOSE: usize = 57;
const SYSCALL_PIPE: usize = 59;
const SYSCALL_READ: usize = 63;
const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;
const SYSCALL_GET_TIME: usize = 169;
const SYSCALL_GETPID: usize = 172;
const SYSCALL_FORK: usize = 220;
const SYSCALL_EXEC: usize = 221;
const SYSCALL_WAITPID: usize = 260;

const SYSCALL_MKDIR: usize = 1024;
const SYSCALL_CD: usize = 1025;
const SYSCALL_LS: usize = 1026;
const SYSCALL_RM: usize = 1027;
const SYSCALL_MV: usize = 1028;

use fs::*;
use process::*;

mod fs;
mod process;

pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    match syscall_id {
        SYSCALL_DUP => sys_dup(args[0]),
        SYSCALL_OPEN => sys_open(args[0], args[1] as *const u8, args[2] as u32),
        SYSCALL_CLOSE => sys_close(args[0]),
        SYSCALL_READ => sys_read(args[0], args[1] as *mut u8, args[2]),
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_GET_TIME => sys_get_time(),
        SYSCALL_GETPID => sys_getpid(),
        SYSCALL_PIPE => sys_pipe(args[0] as *mut usize),
        SYSCALL_FORK => sys_fork(),
        SYSCALL_EXEC => sys_exec(args[0], args[1] as *const u8, args[2] as *const usize),
        SYSCALL_WAITPID => sys_waitpid(args[0] as isize, args[1] as *mut i32),
        SYSCALL_CD => sys_cd(args[0], args[1] as *const u8),
        SYSCALL_LS => sys_ls(args[0]),
        SYSCALL_MV => sys_mv(args[0], args[1] as *const u8, args[2] as *const u8),
        SYSCALL_RM => sys_rm(args[0], args[1] as *const u8),
        SYSCALL_MKDIR => sys_mkdir(args[0], args[1] as *const u8),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
