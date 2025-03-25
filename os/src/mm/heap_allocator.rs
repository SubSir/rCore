use crate::{config::*, console::print};
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

#[link_section = ".heap"]
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];
static mut SIZE_TABEL: [usize; NODE_WIDTH + 1] = [0; NODE_WIDTH + 1];
static mut NODE_LIST: [Node; 2 * NODE_SIZE] = [Node { next: 0 }; 2 * NODE_SIZE];
pub struct Buddy;
#[derive(Copy, Clone)]
struct Node {
    next: usize,
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

    unsafe fn print_node_list(&self) {
        let mut i = KERNEL_HEAP_SIZE;
        let mut j: usize = 0;
        while i >= BLOCK_SIZE {
            let mut head = SIZE_TABEL[j];
            if head != 0 {
                print!("size: {:x}: ", i);
                while head != 0 {
                    print!("{} -> ", head);
                    head = NODE_LIST[head].next;
                }
                println!("");
            }
            i /= 2;
            j += 1;
        }
    }
}
unsafe impl GlobalAlloc for Buddy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        // ignore align!
        // self.print_node_list();
        let mut size = _layout.size();
        // println!("alloc: {}", size);
        if size % BLOCK_SIZE == 0 {
            size /= BLOCK_SIZE;
        } else {
            size = size / BLOCK_SIZE + 1;
        }
        // let mut _align = _layout.align();
        // println!("align: {}", align);
        // if align < BLOCK_SIZE {
        //     align = 1;
        // } else {
        //     align /= BLOCK_SIZE;
        // }
        let mut log = self.log_2(size);
        if (1 << log) < size {
            log += 1;
            size = 1 << log;
        }
        let width = NODE_WIDTH - log;
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
        for j in (i + 1) as usize..=width {
            SIZE_TABEL[j] = father * 2 + 1;
            father = father * 2;
            NODE_LIST[father + 1].next = 0;
        }
        // println!("father: {}", father);
        let addr = (father - (1 << width)) * BLOCK_SIZE * size;
        let base_ptr = HEAP_SPACE.as_mut_ptr();
        let target_ptr = base_ptr.add(addr);
        // println!(
        //     "[buddy] size: {:x}, target_ptr: {:x}, base_ptr: {:x}, allocated: {:x}, node: {}",
        //     _layout.size(),
        //     target_ptr as usize,
        //     base_ptr as usize,
        //     size * BLOCK_SIZE,
        //     father
        // );
        target_ptr
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut size = _layout.size();
        if size % BLOCK_SIZE == 0 {
            size /= BLOCK_SIZE;
        } else {
            size = size / BLOCK_SIZE + 1;
        }
        let mut log = self.log_2(size);
        if (1 << log) < size {
            log += 1;
            size = 1 << log;
        }
        let width = NODE_WIDTH - log;
        let base_ptr = HEAP_SPACE.as_mut_ptr();
        assert!(
            (_ptr as usize - base_ptr as usize) % (BLOCK_SIZE * size) == 0,
            "ptr address not aligned!, {:x}, base_ptr: {:x}, size: {:x}",
            _ptr as usize,
            base_ptr as usize,
            size * BLOCK_SIZE
        );
        let addr = (_ptr as usize - base_ptr as usize) / BLOCK_SIZE / size;
        let node = addr + (1 << width);
        let buddy = node ^ 1;
        // self.print_node_list();
        // println!(
        //     "[buddy] size: {:X}, dealloc: {:x}, node: {}",
        //     _layout.size(),
        //     _ptr as usize,
        //     node,
        // );
        if buddy == 0 {
            SIZE_TABEL[0] = 1;
            return;
        }
        let mut head = SIZE_TABEL[width];
        if head == 0 {
            SIZE_TABEL[width] = node;
            NODE_LIST[node].next = 0;
            return;
        } else if head == buddy {
            SIZE_TABEL[width] = NODE_LIST[buddy].next;
            let ptr_: *mut u8;
            if buddy < node {
                ptr_ = unsafe { _ptr.offset(-((size * BLOCK_SIZE) as isize)) };
            } else {
                ptr_ = _ptr;
            }
            let mut _layout_ =
                Layout::from_size_align_unchecked(size * BLOCK_SIZE * 2, _layout.align());
            self.dealloc(ptr_, _layout_);
        } else if head < buddy {
            while NODE_LIST[head].next != 0 && NODE_LIST[head].next < buddy {
                head = NODE_LIST[head].next;
            }
            if NODE_LIST[head].next == buddy {
                NODE_LIST[head].next = NODE_LIST[buddy].next;
            } else {
                NODE_LIST[node].next = NODE_LIST[head].next;
                NODE_LIST[head].next = node;
                return;
            }
            let ptr_: *mut u8;
            if buddy < node {
                ptr_ = unsafe { _ptr.offset(-((size * BLOCK_SIZE) as isize)) };
            } else {
                ptr_ = _ptr;
            }
            let mut _layout_ =
                Layout::from_size_align_unchecked(size * BLOCK_SIZE * 2, _layout.align());
            self.dealloc(ptr_, _layout_);
        } else {
            NODE_LIST[node].next = head;
            SIZE_TABEL[width] = node;
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
