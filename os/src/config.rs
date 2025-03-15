pub const APP_SIZE_LIMIT: usize = 0x20000;
pub const APP_BASE_ADDRESS: usize = 0x80400000;
pub const MAX_APP_NUM: usize = 4;

pub const USER_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;

pub const CLOCK_FREQ: usize = 12500000;
pub const TICK_PER_SEC: usize = 100;
pub const MICRO_PER_SEC: usize = 1_000;
pub const MTIME_ADDR: usize = 0x0200bff8;
pub const MTIMECMP_ADDR: usize = 0x02004000;
