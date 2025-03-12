use crate::global_asm;
global_asm!(include_str!("switch.S"));

use crate::task::context::TaskContext;

extern "C" {
    pub fn __switch(current_cx_ptr: *mut TaskContext, next_cx_ptr: *const TaskContext);
}
