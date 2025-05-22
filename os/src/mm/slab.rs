use crate::{
    config::{BLOCK_SIZE, KERNEL_HEAP_SIZE, NODE_SIZE, SLAB_PER_BLOCK, SLAB_SIZE},
    mm::heap_allocator::{Buddy, HEAP_SPACE},
};
use alloc::alloc::{GlobalAlloc, Layout};
const SLAB_MEM_COUNT: usize = KERNEL_HEAP_SIZE / SLAB_SIZE;
static mut SLAB_MEM: [u8; SLAB_MEM_COUNT] = [0; SLAB_MEM_COUNT];
static mut PAGE_COUNT: [u8; NODE_SIZE] = [0; NODE_SIZE];
static BUDDY: Buddy = Buddy {};
pub struct Slab;

unsafe impl GlobalAlloc for Slab {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // println!(
        //     "[ALLOC] requested size = {}, align = {}",
        //     layout.size(),
        //     layout.align()
        // );
        if layout.size() > BLOCK_SIZE {
            // println!("[ALLOC] > BLOCK_SIZE, forwarding to BUDDY");
            let ptr = BUDDY.alloc(layout);
            // println!("[ALLOC] after buddy, ptr={:p}", ptr);
            return ptr;
        }
        let slab_count = (layout.size() + SLAB_SIZE - 1) / SLAB_SIZE;
        for i in 0..NODE_SIZE {
            if PAGE_COUNT[i] == 1 {
                let start = i * SLAB_PER_BLOCK;
                let end = (i + 1) * SLAB_PER_BLOCK;
                let slab_range = &SLAB_MEM[start..end];

                let mut found = None;
                let mut zeros = 0;
                let mut begin = 0;
                for (idx, &val) in slab_range.iter().enumerate() {
                    if val == 0 {
                        if zeros == 0 {
                            begin = idx;
                        }
                        zeros += 1;
                        if zeros == slab_count {
                            found = Some(start + begin);
                            break;
                        }
                    } else {
                        zeros = 0;
                    }
                }

                if let Some(pos) = found {
                    let ptr: usize = HEAP_SPACE.as_ptr() as usize + pos * SLAB_SIZE;
                    SLAB_MEM[pos..pos + slab_count]
                        .iter_mut()
                        .for_each(|x| *x = 1);
                    // println!(
                    //     "[ALLOC] In slab page {}, found {} free slabs at offset {}, returning {:p}",
                    //     i, slab_count, pos, ptr as *mut u8
                    // );
                    return ptr as *mut u8;
                }
            }
        }
        let ptr = BUDDY.alloc(Layout::from_size_align(BLOCK_SIZE, layout.align()).unwrap());
        let offset = ptr as usize - HEAP_SPACE.as_ptr() as usize;
        PAGE_COUNT[offset / BLOCK_SIZE] = 1;
        for i in 0..slab_count {
            SLAB_MEM[offset / SLAB_SIZE + i] = 1;
        }
        // println!(
        //     "[ALLOC] No free slab found, allocated new slab page at {:x}, slabs used [{}, {})",
        //     ptr as usize,
        //     offset / SLAB_SIZE,
        //     offset / SLAB_SIZE + slab_count
        // );
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let ptr_usize = ptr as usize;
        let heap_base = HEAP_SPACE.as_ptr() as usize;
        // println!(
        //     "[DEALLOC] ptr = {:p}, size = {}, align = {}",
        //     ptr,
        //     layout.size(),
        //     layout.align()
        // );
        if layout.size() > BLOCK_SIZE {
            // println!("[DEALLOC] Size > BLOCK_SIZE, forwarding to BUDDY");
            BUDDY.dealloc(ptr, layout);
            return;
        }

        let offset = ptr_usize - heap_base;
        let slab_index = offset / SLAB_SIZE;
        let slab_count = (layout.size() + SLAB_SIZE - 1) / SLAB_SIZE;

        for i in 0..slab_count {
            SLAB_MEM[slab_index + i] = 0;
        }

        let page_index = offset / BLOCK_SIZE;
        let page_start = page_index * SLAB_PER_BLOCK;
        let page_end = page_start + SLAB_PER_BLOCK;

        let all_zero = SLAB_MEM[page_start..page_end].iter().all(|&x| x == 0);

        if all_zero {
            PAGE_COUNT[page_index] = 0;
            let page_ptr = HEAP_SPACE.as_ptr().add(page_index * BLOCK_SIZE) as *mut u8;
            BUDDY.dealloc(
                page_ptr,
                Layout::from_size_align(BLOCK_SIZE, layout.align()).unwrap(),
            );
            // println!(
            //     "[DEALLOC] Sla page {} now empty, returned page to BUDDY. PAGE_COUNT[{}]=0",
            //     page_index, page_index
            // );
        }
    }
}

impl Slab {
    pub unsafe fn init(&self) {
        BUDDY.init();
    }
}
