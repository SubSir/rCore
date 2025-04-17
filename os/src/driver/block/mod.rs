pub use virtio_blk::VirtIOBlock;
type BlockDeviceImpl = virtio_blk::VirtIOBlock;
use alloc::sync::Arc;
use easy_fs::BlockDevice;
use lazy_static::*;

mod virtio_blk;

lazy_static! {
    pub static ref BLOCK_DEVICE: Arc<dyn BlockDevice> = Arc::new(BlockDeviceImpl::new());
}
