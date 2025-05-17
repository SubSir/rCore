#![no_std]
#![feature(linkage)]
#![feature(alloc_error_handler)]
#[macro_use]
pub mod console;
mod lang_items;
mod syscall;
extern crate alloc;

use alloc::vec::Vec;
use bitflags::bitflags;
use buddy_system_allocator::LockedHeap;
use core::ptr::addr_of_mut;
use syscall::*;

const USER_HEAP_SIZE: usize = 32768;

static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start(argc: usize, argv: usize) -> ! {
    unsafe {
        HEAP.lock()
            .init(addr_of_mut!(HEAP_SPACE) as usize, USER_HEAP_SIZE);
    }
    let mut v: Vec<&'static str> = Vec::new();
    for i in 0..argc {
        let str_start =
            unsafe { ((argv + i * core::mem::size_of::<usize>()) as *const usize).read_volatile() };
        let len = (0usize..)
            .find(|i| unsafe { ((str_start + *i) as *const u8).read_volatile() == 0 })
            .unwrap();
        v.push(
            core::str::from_utf8(unsafe {
                core::slice::from_raw_parts(str_start as *const u8, len)
            })
            .unwrap(),
        );
    }
    exit(main(argc, v.as_slice()));
}

#[linkage = "weak"]
#[unsafe(no_mangle)]
fn main(_argc: usize, _argv: &[&str]) -> i32 {
    panic!("Cannot find main!");
}

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
    }
}

pub fn open(id: usize, path: &str, flags: OpenFlags) -> isize {
    sys_open(id, path, flags.bits)
}
pub fn close(fd: usize) -> isize {
    sys_close(fd)
}
pub fn read(fd: usize, buffer: &mut [u8]) -> isize {
    sys_read(fd, buffer)
}
pub fn write(fd: usize, buffer: &[u8]) -> isize {
    sys_write(fd, buffer)
}

pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code);
}

pub fn yield_() -> isize {
    sys_yield()
}

pub fn get_time() -> isize {
    sys_get_time()
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn fork() -> isize {
    sys_fork()
}

pub fn exec(id: usize, path: &str, args: &[*const u8]) -> isize {
    sys_exec(id, path, args)
}

pub fn wait(exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(-1, exit_code as *mut _) {
            -2 => {
                yield_();
            }
            // -1 or a valid pid
            exit_pid => return exit_pid,
        }
    }
}

pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(pid as isize, exit_code as *mut _) {
            -2 => {
                yield_();
            }
            // -1 or a valid pid
            exit_pid => return exit_pid,
        }
    }
}

pub fn sleep(period_ms: usize) {
    let start = get_time();
    while get_time() - start < period_ms as isize {
        yield_();
    }
}

pub fn pipe(pipe_fd: &mut [usize]) -> isize {
    sys_pipe(pipe_fd)
}

pub fn dup(fd: usize) -> isize {
    sys_dup(fd)
}

pub fn mkdir(id: usize, name: &str) -> isize {
    sys_mkdir(id, name)
}

pub fn ls(id: usize) -> isize {
    sys_ls(id)
}

pub fn cd(id: usize, path: &str) -> isize {
    sys_cd(id, path)
}

pub fn rm(id: usize, path: &str) -> isize {
    sys_rm(id, path)
}

pub fn mv(id: usize, src: &str, dst: &str) -> isize {
    sys_mv(id, src, dst)
}

pub fn waittid(tid: usize) -> isize {
    loop {
        match sys_waittid(tid) {
            -2 => {
                yield_();
            }
            exit_code => return exit_code,
        }
    }
}

pub fn thread_create(entry: usize, arg: usize) -> isize {
    sys_thread_create(entry, arg)
}

pub fn gettid() -> isize {
    sys_gettid()
}

pub fn mutex_create() -> isize {
    sys_mutex_create(false)
}
pub fn mutex_blocking_create() -> isize {
    sys_mutex_create(true)
}
pub fn mutex_lock(mutex_id: usize) {
    sys_mutex_lock(mutex_id);
}
pub fn mutex_unlock(mutex_id: usize) {
    sys_mutex_unlock(mutex_id);
}

pub fn kill() -> isize {
    sys_kill()
}

#[macro_export]
macro_rules! vstore {
    ($var: expr, $value: expr) => {
        // unsafe { core::intrinsics::volatile_store($var_ref as *const _ as _, $value) }
        unsafe {
            core::ptr::write_volatile(core::ptr::addr_of_mut!($var), $value);
        }
    };
}

#[macro_export]
macro_rules! vload {
    ($var: expr) => {
        // unsafe { core::intrinsics::volatile_load($var_ref as *const _ as _) }
        unsafe { core::ptr::read_volatile(core::ptr::addr_of!($var)) }
    };
}

#[macro_export]
macro_rules! memory_fence {
    () => {
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst)
    };
}
