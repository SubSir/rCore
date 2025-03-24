use crate::config::*;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];
static mut SIZE_TABEL: [usize; NODE_WIDTH + 1] = [0; NODE_WIDTH + 1];
static mut NODE_LIST: [Node; 2 * NODE_SIZE] = [Node {
    next: 0,
    used: false,
}; 2 * NODE_SIZE];
pub struct Buddy;
#[derive(Copy, Clone)]
struct Node {
    next: usize,
    used: bool,
}

impl Buddy {
    pub unsafe fn init(&self) {
        SIZE_TABEL[0] = 1;
    }

    fn log_2(&self, mut x: usize) -> usize {
        assert!(x > 0);
        let mut i = 0;
        while x != 0 {
            x >>= 1;
            i += 1;
        }
        i - 1
    }
}
unsafe impl GlobalAlloc for Buddy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        // ignore align!
        let mut size = _layout.size();
        // println!("alloc: {}", size);
        if size < BLOCK_SIZE {
            size = 1;
        } else {
            size /= BLOCK_SIZE;
        }
        // let mut _align = _layout.align();
        // println!("align: {}", align);
        // if align < BLOCK_SIZE {
        //     align = 1;
        // } else {
        //     align /= BLOCK_SIZE;
        // }
        let width = NODE_WIDTH - self.log_2(size);
        let mut i = width as isize;
        // println!("width: {}, size: {}, align: {}", i, size, align);
        while i >= 0 && SIZE_TABEL[i as usize] == 0 {
            i -= 1;
        }
        if i < 0 {
            return null_mut();
        }
        // println!("i: {}", i);
        let mut father = SIZE_TABEL[i as usize];
        SIZE_TABEL[i as usize] = NODE_LIST[father].next;
        NODE_LIST[father].used = true;
        for j in (i + 1) as usize..=width {
            SIZE_TABEL[j] = father * 2 + 1;
            father = father * 2;
            NODE_LIST[father].used = true;
        }
        // println!("father: {}", father);
        let addr = (father - (1 << width)) * BLOCK_SIZE * size;
        let base_ptr = HEAP_SPACE.as_mut_ptr();
        let target_ptr = base_ptr.add(addr);
        println!(
            "[buddy] size: {:x}, target_ptr: {:x}, base_ptr: {:x}, allocated: {:x}, node: {}",
            _layout.size(),
            target_ptr as usize,
            base_ptr as usize,
            size * BLOCK_SIZE,
            father
        );
        target_ptr
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut size = _layout.size();
        if size < BLOCK_SIZE {
            size = 1;
        } else {
            size /= BLOCK_SIZE;
        }
        let width = NODE_WIDTH - self.log_2(size);
        let base_ptr = HEAP_SPACE.as_mut_ptr();
        let addr = ((_ptr as usize - base_ptr as usize) / BLOCK_SIZE / size) * size;
        let node = addr + (1 << width);
        let buddy = node ^ 1;
        println!(
            "[buddy] size: {:X}, dealloc: {:x}, node: {}",
            _layout.size(),
            _ptr as usize,
            node,
        );
        NODE_LIST[node].used = false;
        if buddy == 0 {
            SIZE_TABEL[0] = 1;
            return;
        }
        if NODE_LIST[buddy].used == false {
            let mut head = SIZE_TABEL[width];
            if head == buddy {
                SIZE_TABEL[width] = NODE_LIST[buddy].next;
            } else {
                while NODE_LIST[head].next != 0 && NODE_LIST[head].next != buddy {
                    head = NODE_LIST[head].next;
                }
                NODE_LIST[head].next = NODE_LIST[buddy].next;
            }
            let mut _layout_ =
                Layout::from_size_align_unchecked(size * BLOCK_SIZE * 2, _layout.align());
            self.dealloc(_ptr, _layout_);
        } else {
            if SIZE_TABEL[width] == 0 {
                SIZE_TABEL[width] = node;
                NODE_LIST[node].next = 0;
                return;
            }
            let mut head = SIZE_TABEL[width];
            if head == 0 {
                SIZE_TABEL[width] = node;
            } else {
                while NODE_LIST[head].next != 0 && NODE_LIST[head].next < node {
                    head = NODE_LIST[head].next;
                }
                NODE_LIST[node].next = NODE_LIST[head].next;
                NODE_LIST[head].next = node;
            }
        }
    }
}
#[global_allocator]
static HEAP_ALLOCATOR: Buddy = Buddy {};
pub fn init_heap() {
    unsafe {
        HEAP_ALLOCATOR.init();
    }
}

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[allow(unused)]
pub fn heap_test() {
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    extern "C" {
        fn sbss();
        fn ebss();
    }
    let bss_range = sbss as usize..ebss as usize;
    let a = Box::new(5);
    assert_eq!(*a, 5);
    assert!(bss_range.contains(&(a.as_ref() as *const _ as usize)));
    drop(a);
    let mut v = Vec::new();
    for i in 0..500 {
        v.push(i);
    }
    for i in 0..500 {
        assert_eq!(v[i], i);
    }
    assert!(bss_range.contains(&(v.as_ptr() as usize)));
    drop(v);
    println!("heap_test passed!");
}
