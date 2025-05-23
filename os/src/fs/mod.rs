use crate::mm::UserBuffer;

mod inode;
mod pipe;
mod stdio;

pub use inode::*;
pub use pipe::make_pipe;
pub use stdio::{Stdin, Stdout};

pub trait File: Send + Sync {
    fn read(&self, buf: UserBuffer) -> usize;
    fn write(&self, buf: UserBuffer) -> usize;
    fn readable(&self) -> bool;
    fn writable(&self) -> bool;
}
