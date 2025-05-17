use crate::{
    fs::*,
    mm::{translate_byte_buffer, translated_refmut, translated_str, UserBuffer},
    task::processor::{current_process, current_user_token},
};
use easy_fs::block_cache_sync_all;

pub fn sys_write(fd: usize, buffer: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        drop(inner);
        let ret = file.write(UserBuffer::new(translate_byte_buffer(token, buffer, len))) as isize;
        block_cache_sync_all();
        ret
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buffer: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        drop(inner);
        file.read(UserBuffer::new(translate_byte_buffer(token, buffer, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(id: usize, path: *const u8, flags: u32) -> isize {
    let process = current_process();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(id, path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = process.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        block_cache_sync_all();
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

pub fn sys_pipe(pipe: *mut usize) -> isize {
    let process = current_process();
    let token = current_user_token();
    let mut inner = process.inner_exclusive_access();
    let (pipe_read, pipe_write) = make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);
    *translated_refmut(token, pipe) = read_fd;
    *translated_refmut(token, unsafe { pipe.add(1) }) = write_fd;
    block_cache_sync_all();
    0
}

pub fn sys_dup(fd: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    let new_fd = inner.alloc_fd();
    inner.fd_table[new_fd] = Some(inner.fd_table[fd].clone().unwrap());
    block_cache_sync_all();
    new_fd as isize
}

pub fn sys_mkdir(id: usize, path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    let ret = mkdir(id, path.as_str());
    block_cache_sync_all();
    ret
}

pub fn sys_ls(id: usize) -> isize {
    ls(id)
}

pub fn sys_cd(id: usize, path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    cd(id, path.as_str())
}

pub fn sys_rm(id: usize, path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    let ret = rm(id, path.as_str());
    block_cache_sync_all();
    ret
}

pub fn sys_mv(id: usize, src: *const u8, dst: *const u8) -> isize {
    let token = current_user_token();
    let src = translated_str(token, src);
    let dst = translated_str(token, dst);
    let ret = mv(id, src.as_str(), dst.as_str());
    block_cache_sync_all();
    ret
}
