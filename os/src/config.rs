pub const APP_SIZE_LIMIT: usize = 0x20000;
pub const APP_BASE_ADDRESS: usize = 0x80400000;
pub const MAX_APP_NUM: usize = 4;

pub const USER_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_HEAP_SIZE: usize = 1 << KERNEL_HEAP_WIDTH;
pub const KERNEL_HEAP_WIDTH: usize = 21;
pub const NODE_WIDTH: usize = 13;
pub const BLOCK_SIZE: usize = 1 << (KERNEL_HEAP_WIDTH - NODE_WIDTH);
pub const NODE_SIZE: usize = 1 << NODE_WIDTH;
pub const MEMORY_END: usize = 0x80800000;

pub const CLOCK_FREQ: usize = 12500000;
pub const TICK_PER_SEC: usize = 100;
pub const MICRO_PER_SEC: usize = 1_000;
pub const MTIME_ADDR: usize = 0x0200bff8;
pub const MTIMECMP_ADDR: usize = 0x02004000;

pub const PAGE_SIZE_BITS: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS;
pub const PA_WIDTH_SV39: usize = 56;
pub const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - PAGE_SIZE_BITS;
pub const VA_WIDTH_SV39: usize = 39;
pub const VPN_WIDTH_SV39: usize = VA_WIDTH_SV39 - PAGE_SIZE_BITS;
