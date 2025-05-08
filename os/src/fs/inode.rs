use crate::driver::BLOCK_DEVICE;
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::bitflags;
use easy_fs::EasyFileSystem;
use easy_fs::Inode;
use lazy_static::lazy_static;

use crate::mm::UserBuffer;

use super::File;

pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPSafeCell<OSInodeInner>,
}

pub struct OSInodeInner {
    offset: usize,
    inode: Arc<Inode>,
}

impl OSInode {
    pub fn new(readable: bool, writable: bool, inode: Arc<Inode>) -> Self {
        Self {
            readable,
            writable,
            inner: unsafe { UPSafeCell::new(OSInodeInner { offset: 0, inode }) },
        }
    }

    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buf = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buf);
            if len == 0 {
                break;
            }
            inner.offset += len;
            v.extend_from_slice(&buf[..len]);
        }
        v
    }
}

impl File for OSInode {
    fn readable(&self) -> bool {
        self.readable
    }

    fn writable(&self) -> bool {
        self.writable
    }

    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        for slice in buf.buffers.iter_mut() {
            let read_size = inner.inode.read_at(inner.offset, *slice);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }

    fn write(&self, buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.offset, *slice);
            assert_eq!(write_size, slice.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
}

lazy_static! {
    pub static ref ROOT_INODE: Arc<Inode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

pub fn list_apps() {
    println!("/**** APPS ****/");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("/**************/");
}

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
    }
}

impl OpenFlags {
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

pub fn open_file(id: usize, name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();
    let root_inode = Arc::new(ROOT_INODE.get_inode(id as u32));
    if flags.contains(OpenFlags::CREATE) {
        if let Some(inode) = root_inode.find(name) {
            inode.clear();
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        } else {
            root_inode
                .create(name)
                .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
        }
    } else {
        root_inode.find(name).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            Arc::new(OSInode::new(readable, writable, inode))
        })
    }
}

pub fn mkdir(id: usize, name: &str) -> isize {
    let node = ROOT_INODE.get_inode(id as u32).mkdir(name);
    if node.is_none() {
        -1
    } else {
        node.unwrap().self_id() as isize
    }
}

pub fn ls(id: usize) -> isize {
    let node = ROOT_INODE.get_inode(id as u32);
    for name in node.ls() {
        println!("{}", name);
    }
    0
}

pub fn cd(id: usize, path: &str) -> isize {
    let node = ROOT_INODE.get_inode(id as u32).cd(path);
    if node.is_none() {
        -1
    } else {
        node.unwrap().self_id() as isize
    }
}

pub fn rm(id: usize, path: &str) -> isize {
    let node = ROOT_INODE.get_inode(id as u32);
    if node.remove(path) {
        0
    } else {
        -1
    }
}
pub fn mv(id: usize, src: &str, dst: &str) -> isize {
    let node = ROOT_INODE.get_inode(id as u32);
    if node.mv(src, dst) {
        0
    } else {
        -1
    }
}
