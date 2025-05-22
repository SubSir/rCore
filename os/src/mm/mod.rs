mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;
mod slab;
pub use address::*;
pub use frame_allocator::*;
pub use memory_set::*;
pub use page_table::*;

pub use memory_set::KERNEL_SPACE;

pub fn init() {
    heap_allocator::init_heap();
    // heap_allocator::heap_test();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activate();
}
