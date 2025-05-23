mod context;

use crate::task::processor::{current_trap_cx_user_va, current_user_token};
use crate::{
    config::{TRAMPOLINE, TRAP_CONTEXT},
    syscall::syscall,
    task::{processor::current_trap_cx, suspend_current_and_run_next},
};
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
pub fn trap_handler() -> ! {
    set_kernel_entry();
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            let mut cx = current_trap_cx();
            cx.sepc += 4;
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
            cx = current_trap_cx();
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            println!("[kernel] PageFault in application, kernel killed it.");
            exit_current_and_run_next(-2);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            exit_current_and_run_next(-3);
        }
        Trap::Interrupt(Interrupt::SupervisorSoft) => {
            // println!("[Timer] interrupt in application, kernel suspend it.");
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
    trap_return();
}

pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

fn set_kernel_entry() {
    unsafe {
        stvec::write(trap_from_kernel as usize, stvec::TrapMode::Direct);
    }
}

#[no_mangle]
pub fn trap_from_kernel() -> ! {
    panic!("a trap from kernel!");
}

fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

#[no_mangle]
pub fn trap_return() -> ! {
    set_user_trap_entry();
    let trap_cx_user_va = current_trap_cx_user_va();
    let user_satp = current_user_token();
    extern "C" {
        fn __alltraps();
        fn __restore();
    }
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE;
    unsafe {
        asm!(
            "fence.i",
            "jr {restore_va}",
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_user_va,
            in("a1") user_satp,
            options(noreturn)
        );
    }
}
