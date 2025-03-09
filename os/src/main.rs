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

use core::arch::global_asm;

use sbi::Memory_Managr;

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
