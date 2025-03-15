mod context;

use crate::{syscall::syscall, task::suspend_current_and_run_next};
pub use context::TrapContext;
use core::arch::{asm, global_asm};

global_asm!(include_str!("trap.S"));
use crate::task::exit_current_and_run_next;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, sip, stval, stvec,
};
#[no_mangle]
pub fn init_() {
    extern "C" {
        fn __alltraps();
    }

    unsafe {
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

#[unsafe(no_mangle)]
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            cx.sepc += 4;
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        }
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) => {
            println!("[kernel] PageFault in application, kernel killed it.");
            exit_current_and_run_next();
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            exit_current_and_run_next();
        }
        Trap::Interrupt(Interrupt::SupervisorSoft) => {
            println!("[Timer] interrupt in application, kernel suspend it.");
            let mut sip = sip::read().bits();
            sip = sip & !(1 << 1);
            unsafe {
                asm!("csrw sip, {}", in(reg) sip);
            }
            suspend_current_and_run_next();
            // println!("[Timer] interrupt handled");
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    cx
}

pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}
