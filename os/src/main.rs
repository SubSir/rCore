// os/src/main.rs
#![no_main]
#![no_std]
#![feature(alloc_error_handler)]

#[macro_use]

mod console;
mod config;
mod driver;
mod fs;
mod lang_items;
mod mm;
mod sbi;
mod sync;
pub mod syscall;
mod task;
pub mod trap;
use config::*;
use core::arch::asm;
mod timer;
use core::arch::global_asm;
use riscv::register::{mepc, mhartid, mie, mscratch, mstatus, mtvec, pmpaddr0, pmpcfg0, satp, sie};

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));
global_asm!(include_str!("time_handler.S"));

extern crate alloc;
extern crate bitflags;

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    println!("[kernel] Hello, world!");
    mm::init();
    println!("[kernel] memory init");
    mm::remap_test();
    trap::init_();
    fs::list_apps();
    task::add_initproc();
    println!("after initproc!");
    task::processor::run_tasks();
    panic!("Unreachable in rust_main!");
}

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) });
}

#[no_mangle]
unsafe fn init() -> ! {
    mstatus::set_mpp(mstatus::MPP::Supervisor);
    mepc::write(rust_main as usize);
    satp::write(0);
    asm!("csrw medeleg, {}", in(reg) 0xffff);
    asm!("csrw mideleg, {}", in(reg) 0xffff);
    sie::set_ssoft();
    sie::set_sext();
    sie::set_stimer();
    pmpaddr0::write(0x3fffffffffffff);
    pmpcfg0::write(0xf);
    time_init();
    asm!("mret", options(noreturn),)
}

unsafe fn time_init() {
    let hartid = mhartid::read();
    use crate::sbi::set_timer;
    set_timer(hartid, CLOCK_FREQ / TICK_PER_SEC + timer::get_time());
    extern "C" {
        fn __timer_scratch();
    }
    mscratch::write(__timer_scratch as usize);
    extern "C" {
        fn __timehandler();
    }
    mtvec::write(__timehandler as usize, mtvec::TrapMode::Direct);
    mstatus::set_mie();
    mie::set_mtimer();
}
