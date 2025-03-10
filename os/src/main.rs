// os/src/main.rs
#![no_main]
#![no_std]
#[macro_use]

mod console;
mod batch;
mod lang_items;
mod sbi;
mod sync;
pub mod syscall;
pub mod trap;

use core::arch::asm;
use core::arch::global_asm;
use riscv::register::{mepc, mstatus, pmpaddr0, pmpcfg0, satp, sie};

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    println!("[kernel] Hello, world!");
    trap::init();
    batch::init();
    batch::run_next_app();
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
    asm!("mret", options(noreturn),)
}
