use super::id::pid_alloc;
use super::id::RecycleAllocator;
use super::manager::add_task;
use super::manager::insert_into_pid2process;
use crate::fs::File;
use crate::fs::*;
use crate::mm::translated_refmut;
use crate::mm::MemorySet;
use crate::mm::KERNEL_SPACE;
use crate::sync::Condvar;
use crate::sync::Mutex;
use crate::task::id::PidHandle;
use crate::task::TaskControlBlock;
use crate::task::UPSafeCell;
use crate::trap::trap_handler;
use crate::trap::TrapContext;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;
pub struct ProcessControlBlock {
    pub pid: PidHandle,
    inner: UPSafeCell<ProcessControlBlockInner>,
}

impl ProcessControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, ProcessControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }

    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        let pid_handle = pid_alloc();
        let process = Arc::new(Self {
            pid: pid_handle,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    fd_table: vec![
                        Some(Arc::new(Stdin)),
                        Some(Arc::new(Stdout)),
                        Some(Arc::new(Stdout)),
                    ],
                    exit_code: 0,
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    mutex_list: Vec::new(),
                    condvar_list: Vec::new(),
                })
            },
        });
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&process),
            ustack_base,
            true,
        ));
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        let ustack_top = task_inner.res.as_ref().unwrap().ustack_top();
        let kstack_top = task.kernel_stack.get_top();
        drop(task_inner);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_top,
            KERNEL_SPACE.exclusive_access().token(),
            kstack_top,
            trap_handler as usize,
        );
        let mut process_inner = process.inner_exclusive_access();
        process_inner.tasks.push(Some(Arc::clone(&task)));
        drop(process_inner);
        insert_into_pid2process(process.getpid(), Arc::clone(&process));
        add_task(task);
        process
    }

    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        assert_eq!(self.inner_exclusive_access().thread_count(), 1);
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        let new_token = memory_set.token();
        self.inner_exclusive_access().memory_set = memory_set;
        let task = self.inner_exclusive_access().get_task(0);
        let mut task_inner = task.inner_exclusive_access();
        task_inner.res.as_mut().unwrap().ustack_base = ustack_base;
        task_inner.res.as_mut().unwrap().alloc_user_res();
        task_inner.trap_cx_ppn = task_inner.res.as_mut().unwrap().trap_cx_ppn();
        let mut user_sp = task_inner.res.as_mut().unwrap().ustack_top();
        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        let mut argv: Vec<_> = (0..=args.len())
            .map(|arg| {
                translated_refmut(
                    new_token,
                    (argv_base + arg * core::mem::size_of::<usize>()) as *mut usize,
                )
            })
            .collect();
        *argv[args.len()] = 0;
        for i in 0..args.len() {
            user_sp -= args[i].len() + 1;
            *argv[i] = user_sp;
            let mut p = user_sp;
            for c in args[i].as_bytes() {
                *translated_refmut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            *translated_refmut(new_token, p as *mut u8) = 0;
        }
        user_sp -= user_sp % core::mem::size_of::<usize>();
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            task.kernel_stack.get_top(),
            trap_handler as usize,
        );
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = argv_base;
        *task_inner.get_trap_cx() = trap_cx;
    }

    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent = self.inner_exclusive_access();
        assert_eq!(parent.thread_count(), 1);
        let memory_set = MemorySet::from_existed_user(&parent.memory_set);
        let pid = pid_alloc();
        let mut new_fd_table = Vec::<Option<Arc<dyn File + Send + Sync>>>::new();
        for fd in parent.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        let child = Arc::new(Self {
            pid,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    fd_table: new_fd_table,
                    exit_code: 0,
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    mutex_list: Vec::new(),
                    condvar_list: Vec::new(),
                })
            },
        });
        parent.children.push(Arc::clone(&child));
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&child),
            parent
                .get_task(0)
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .ustack_base(),
            false,
        ));

        let mut child_inner = child.inner_exclusive_access();
        child_inner.tasks.push(Some(Arc::clone(&task)));
        drop(child_inner);
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        trap_cx.kernel_sp = task.kernel_stack.get_top();
        drop(task_inner);
        insert_into_pid2process(child.getpid(), Arc::clone(&child));
        add_task(task);
        child
    }
}

pub struct ProcessControlBlockInner {
    pub is_zombie: bool,
    pub memory_set: MemorySet,
    pub parent: Option<Weak<ProcessControlBlock>>,
    pub children: Vec<Arc<ProcessControlBlock>>,
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    pub exit_code: i32,
    pub tasks: Vec<Option<Arc<TaskControlBlock>>>,
    pub task_res_allocator: RecycleAllocator,
    pub mutex_list: Vec<Option<Arc<dyn Mutex>>>,
    pub condvar_list: Vec<Option<Arc<Condvar>>>,
}

impl ProcessControlBlockInner {
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }

    pub fn alloc_tid(&mut self) -> usize {
        self.task_res_allocator.alloc()
    }

    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_res_allocator.dealloc(tid);
    }

    pub fn thread_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn get_task(&self, tid: usize) -> Arc<TaskControlBlock> {
        self.tasks[tid].as_ref().unwrap().clone()
    }
}
