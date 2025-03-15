use riscv::register::time;
#[no_mangle]
pub fn get_time() -> usize {
    // println!("get_time: {}", t);
    unsafe { (0x0200bff8 as *const usize).read_volatile() }
}

use crate::config::*;

#[no_mangle]
pub fn get_time_ms() -> usize {
    let t = time::read() / (CLOCK_FREQ / MICRO_PER_SEC);
    // println!("get_time_us: {}", t);
    t
}
