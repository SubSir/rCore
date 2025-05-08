#![no_std]
extern crate alloc;
mod bitmap;
mod block_cache;
mod block_dev;
mod efs;
mod layout;
mod vfs;

pub const BLOCK_SZ: usize = 512;

pub use crate::block_cache::block_cache_sync_all;
pub use crate::block_dev::BlockDevice;
pub use crate::efs::EasyFileSystem;
pub use crate::vfs::Inode;
