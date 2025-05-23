use alloc::{sync::Arc, vec::Vec};

use crate::{BLOCK_SZ, block_cache::get_block_cache, block_dev::BlockDevice};
const EFS_MAGIC: u32 = 0x3b800001;
const INODE_DIRECT_COUNT: usize = 28;
const DIRECT_BOUND: usize = INODE_DIRECT_COUNT;
const INODE_INDIRECT1_COUNT: usize = BLOCK_SZ / 4;
const INODE_INDIRECT2_COUNT: usize = INODE_INDIRECT1_COUNT * INODE_INDIRECT1_COUNT;
const INDIRECT1_BOUND: usize = INODE_INDIRECT1_COUNT + DIRECT_BOUND;
const NAME_LENGTH_LIMIT: usize = 27;
pub const DIRENT_SZ: usize = 32;
type IndirectBlock = [u32; BLOCK_SZ / 4];
type DataBlock = [u8; BLOCK_SZ];

#[repr(C)]
pub struct SuperBlock {
    magic: u32,
    pub total_blocks: u32,
    pub inode_bitmap_blocks: u32,
    pub inode_area_blocks: u32,
    pub data_bitmap_blocks: u32,
    pub data_area_blocks: u32,
}

impl SuperBlock {
    pub fn initialize(
        &mut self,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
        inode_area_blocks: u32,
        data_bitmap_blocks: u32,
        data_area_blocks: u32,
    ) {
        *self = Self {
            magic: EFS_MAGIC,
            total_blocks,
            inode_bitmap_blocks,
            inode_area_blocks,
            data_bitmap_blocks,
            data_area_blocks,
        };
    }

    pub fn is_valid(&self) -> bool {
        self.magic == EFS_MAGIC
    }
}

#[repr(C)]
pub struct DiskInode {
    pub size: u32,
    pub direct: [u32; INODE_DIRECT_COUNT],
    pub indirect1: u32,
    pub indirect2: u32,
    pub parent: u32,
    type_: DiskInodeType,
}

#[derive(PartialEq)]
pub enum DiskInodeType {
    File,
    Directory,
}

impl DiskInode {
    pub fn initialize(&mut self, type_: DiskInodeType) {
        self.size = 0;
        self.direct.iter_mut().for_each(|v| *v = 0);
        self.indirect1 = 0;
        self.indirect2 = 0;
        self.type_ = type_;
        self.parent = 0;
    }

    pub fn is_dir(&self) -> bool {
        self.type_ == DiskInodeType::Directory
    }

    pub fn is_file(&self) -> bool {
        self.type_ == DiskInodeType::File
    }

    pub fn get_block_id(&self, inner_id: u32, block_device: &Arc<dyn BlockDevice>) -> u32 {
        let inner_id = inner_id as usize;
        if inner_id < INODE_DIRECT_COUNT {
            self.direct[inner_id]
        } else if inner_id < INDIRECT1_BOUND {
            get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect_block: &IndirectBlock| {
                    indirect_block[inner_id - INODE_DIRECT_COUNT]
                })
        } else {
            let last = inner_id - INDIRECT1_BOUND;
            let indirect1 = get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect2: &IndirectBlock| {
                    indirect2[last / INODE_INDIRECT1_COUNT]
                });
            get_block_cache(indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect_block: &IndirectBlock| {
                    indirect_block[last % INODE_INDIRECT1_COUNT]
                })
        }
    }

    pub fn data_blocks(&self) -> u32 {
        Self::_data_blocks(self.size)
    }

    fn _data_blocks(size: u32) -> u32 {
        (size + BLOCK_SZ as u32 - 1) / BLOCK_SZ as u32
    }

    pub fn total_blocks(size: u32) -> u32 {
        let data_blocks = Self::_data_blocks(size) as usize;
        let mut total = data_blocks;
        if data_blocks > INODE_DIRECT_COUNT {
            total += 1;
        }
        if data_blocks > INDIRECT1_BOUND {
            total += 1;
            total +=
                (data_blocks - INDIRECT1_BOUND + INODE_INDIRECT1_COUNT - 1) / INODE_INDIRECT1_COUNT;
        }
        total as u32
    }

    pub fn blocks_num_needed(&self, new_size: u32) -> u32 {
        assert!(new_size >= self.size);
        Self::total_blocks(new_size) - Self::total_blocks(self.size)
    }

    pub fn increase_size(
        &mut self,
        new_size: u32,
        new_blocks: Vec<u32>,
        block_device: &Arc<dyn BlockDevice>,
    ) {
        let mut current_blocks = self.data_blocks();
        self.size = new_size;
        let mut total_blocks = self.data_blocks();
        let mut new_blocks = new_blocks.into_iter();
        while current_blocks < total_blocks.min(INODE_DIRECT_COUNT as u32) {
            self.direct[current_blocks as usize] = new_blocks.next().unwrap();
            current_blocks += 1;
        }
        if total_blocks > INODE_DIRECT_COUNT as u32 {
            if current_blocks == INODE_DIRECT_COUNT as u32 {
                self.indirect1 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_DIRECT_COUNT as u32;
            total_blocks -= INODE_DIRECT_COUNT as u32;
        } else {
            return;
        }
        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect_block: &mut IndirectBlock| {
                while current_blocks < total_blocks.min(INODE_INDIRECT1_COUNT as u32) {
                    indirect_block[current_blocks as usize] = new_blocks.next().unwrap();
                    current_blocks += 1;
                }
            });
        if total_blocks > INODE_INDIRECT1_COUNT as u32 {
            if current_blocks == INODE_INDIRECT1_COUNT as u32 {
                self.indirect2 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_INDIRECT1_COUNT as u32;
            total_blocks -= INODE_INDIRECT1_COUNT as u32;
        } else {
            return;
        }
        let mut a0 = current_blocks as usize / INODE_INDIRECT1_COUNT;
        let mut b0 = current_blocks as usize % INODE_INDIRECT1_COUNT;
        let a1 = total_blocks as usize / INODE_INDIRECT1_COUNT;
        let b1 = total_blocks as usize % INODE_INDIRECT1_COUNT;
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect2: &mut IndirectBlock| {
                while (a0 < a1) || (a0 == a1 && b0 < b1) {
                    if b0 == 0 {
                        indirect2[a0] = new_blocks.next().unwrap();
                    }
                    get_block_cache(indirect2[a0] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect_block: &mut IndirectBlock| {
                            indirect_block[b0] = new_blocks.next().unwrap();
                        });
                    b0 += 1;
                    if b0 == INODE_INDIRECT1_COUNT {
                        b0 = 0;
                        a0 += 1;
                    }
                }
            })
    }

    pub fn clear_size(&mut self, block_device: &Arc<dyn BlockDevice>) -> Vec<u32> {
        let mut v: Vec<u32> = Vec::new();
        let mut data_blocks = self.data_blocks() as usize;
        self.size = 0;
        let mut current_blocks = 0usize;
        while current_blocks < data_blocks.min(INODE_DIRECT_COUNT) {
            v.push(self.direct[current_blocks]);
            self.direct[current_blocks] = 0;
            current_blocks += 1;
        }

        if data_blocks > INODE_DIRECT_COUNT {
            v.push(self.indirect1);
            data_blocks -= INODE_DIRECT_COUNT;
            current_blocks = 0;
        } else {
            return v;
        }

        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect_block: &mut IndirectBlock| {
                while current_blocks < data_blocks.min(INODE_INDIRECT1_COUNT) {
                    v.push(indirect_block[current_blocks]);
                    indirect_block[current_blocks] = 0;
                    current_blocks += 1;
                }
            });
        self.indirect1 = 0;
        if data_blocks > INODE_INDIRECT1_COUNT {
            v.push(self.indirect2);
            data_blocks -= INODE_INDIRECT1_COUNT;
        } else {
            return v;
        }
        assert!(data_blocks <= INODE_INDIRECT2_COUNT);
        let a1 = data_blocks / INODE_INDIRECT1_COUNT;
        let b1 = data_blocks % INODE_INDIRECT1_COUNT;
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect2: &mut IndirectBlock| {
                for entry in indirect2.iter_mut().take(a1) {
                    v.push(*entry);
                    get_block_cache(*entry as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter() {
                                v.push(*entry);
                            }
                        });
                }
                if b1 > 0 {
                    v.push(indirect2[a1]);
                    get_block_cache(indirect2[a1] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter().take(b1) {
                                v.push(*entry);
                            }
                        });
                }
            });
        self.indirect2 = 0;
        v
    }

    pub fn decrease_size(
        &mut self,
        new_size: u32,
        block_device: &Arc<dyn BlockDevice>,
    ) -> Vec<u32> {
        // Calculate old and new block counts
        let old_blocks = self.data_blocks();
        self.size = new_size;
        let new_blocks = self.data_blocks();

        if new_blocks >= old_blocks {
            return Vec::new();
        }

        let mut freed_blocks = Vec::new();
        let mut current_blocks = new_blocks as usize;
        let mut total_blocks = old_blocks as usize;

        while current_blocks < total_blocks.min(INODE_DIRECT_COUNT) {
            freed_blocks.push(self.direct[current_blocks]);
            self.direct[current_blocks] = 0;
            current_blocks += 1;
        }

        if total_blocks > INODE_DIRECT_COUNT {
            if current_blocks == INODE_DIRECT_COUNT {
                if new_blocks <= INODE_DIRECT_COUNT as u32 {
                    freed_blocks.push(self.indirect1);
                    get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect_block: &mut IndirectBlock| {
                            for block in indirect_block
                                .iter()
                                .take(total_blocks - INODE_DIRECT_COUNT)
                            {
                                freed_blocks.push(*block);
                            }
                        });
                    self.indirect1 = 0;
                    return freed_blocks;
                }
                current_blocks = 0;
                total_blocks -= INODE_DIRECT_COUNT;
            } else {
                return freed_blocks;
            }
        } else {
            return freed_blocks;
        }

        if total_blocks > 0 {
            get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
                .lock()
                .modify(0, |indirect_block: &mut IndirectBlock| {
                    while current_blocks < total_blocks.min(INODE_INDIRECT1_COUNT) {
                        freed_blocks.push(indirect_block[current_blocks]);
                        indirect_block[current_blocks] = 0;
                        current_blocks += 1;
                    }
                });

            if total_blocks > INODE_INDIRECT1_COUNT {
                if current_blocks == INODE_INDIRECT1_COUNT {
                    if new_blocks <= (INODE_DIRECT_COUNT + INODE_INDIRECT1_COUNT) as u32 {
                        freed_blocks.push(self.indirect2);
                        let a1 = total_blocks / INODE_INDIRECT1_COUNT;
                        let b1 = total_blocks % INODE_INDIRECT1_COUNT;

                        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
                            .lock()
                            .modify(0, |indirect2: &mut IndirectBlock| {
                                for entry in indirect2.iter().take(a1) {
                                    freed_blocks.push(*entry);
                                    get_block_cache(*entry as usize, Arc::clone(block_device))
                                        .lock()
                                        .modify(0, |indirect1: &mut IndirectBlock| {
                                            for block in indirect1.iter() {
                                                freed_blocks.push(*block);
                                            }
                                        });
                                }
                                if b1 > 0 {
                                    freed_blocks.push(indirect2[a1]);
                                    get_block_cache(
                                        indirect2[a1] as usize,
                                        Arc::clone(block_device),
                                    )
                                    .lock()
                                    .modify(
                                        0,
                                        |indirect1: &mut IndirectBlock| {
                                            for block in indirect1.iter().take(b1) {
                                                freed_blocks.push(*block);
                                            }
                                        },
                                    );
                                }
                            });
                        self.indirect2 = 0;
                        return freed_blocks;
                    }
                    current_blocks = 0;
                    total_blocks -= INODE_INDIRECT1_COUNT;
                } else {
                    return freed_blocks;
                }
            } else {
                return freed_blocks;
            }
        } else {
            return freed_blocks;
        }

        if total_blocks > 0 {
            let a0 = current_blocks / INODE_INDIRECT1_COUNT;
            let b0 = current_blocks % INODE_INDIRECT1_COUNT;
            let a1 = total_blocks / INODE_INDIRECT1_COUNT;
            let b1 = total_blocks % INODE_INDIRECT1_COUNT;

            get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
                .lock()
                .modify(0, |indirect2: &mut IndirectBlock| {
                    if a0 < a1 {
                        for entry in indirect2.iter_mut().skip(a0 + 1).take(a1 - a0 - 1) {
                            freed_blocks.push(*entry);
                            get_block_cache(*entry as usize, Arc::clone(block_device))
                                .lock()
                                .modify(0, |indirect1: &mut IndirectBlock| {
                                    for block in indirect1.iter() {
                                        freed_blocks.push(*block);
                                    }
                                });
                            *entry = 0;
                        }

                        if b0 > 0 {
                            freed_blocks.push(indirect2[a0]);
                            get_block_cache(indirect2[a0] as usize, Arc::clone(block_device))
                                .lock()
                                .modify(0, |indirect1: &mut IndirectBlock| {
                                    for block in indirect1.iter().skip(b0) {
                                        freed_blocks.push(*block);
                                    }
                                });
                            indirect2[a0] = 0;
                        }

                        if b1 > 0 {
                            freed_blocks.push(indirect2[a1]);
                            get_block_cache(indirect2[a1] as usize, Arc::clone(block_device))
                                .lock()
                                .modify(0, |indirect1: &mut IndirectBlock| {
                                    for block in indirect1.iter().skip(b1) {
                                        freed_blocks.push(*block);
                                    }
                                });
                            indirect2[a1] = 0;
                        }
                    } else if b0 < b1 {
                        get_block_cache(indirect2[a0] as usize, Arc::clone(block_device))
                            .lock()
                            .modify(0, |indirect1: &mut IndirectBlock| {
                                for block in indirect1.iter_mut().skip(b0).take(b1 - b0) {
                                    freed_blocks.push(*block);
                                    *block = 0;
                                }
                            });
                    }
                });

            if new_blocks <= (INODE_DIRECT_COUNT + INODE_INDIRECT1_COUNT) as u32 {
                freed_blocks.push(self.indirect2);
                self.indirect2 = 0;
            }
        }

        freed_blocks
    }

    pub fn read_at(
        &self,
        offset: usize,
        buf: &mut [u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start = offset;
        let end = (offset + buf.len()).min(self.size as usize);
        if start >= end {
            return 0;
        }
        let mut start_block = start / BLOCK_SZ;
        let mut read_size = 0usize;
        loop {
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            end_current_block = end_current_block.min(end);
            let block_read_size = end_current_block - start;
            let dst = &mut buf[read_size..read_size + block_read_size];
            get_block_cache(
                self.get_block_id(start_block as u32, block_device) as usize,
                Arc::clone(block_device),
            )
            .lock()
            .read(0, |data_block: &DataBlock| {
                let src = &data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_read_size];
                dst.copy_from_slice(src);
            });
            read_size += block_read_size;
            if end_current_block >= end {
                break;
            }
            start_block += 1;
            start = end_current_block;
        }
        read_size
    }

    pub fn write_at(
        &mut self,
        offset: usize,
        buf: &[u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start = offset;
        let end = (offset + buf.len()).min(self.size as usize);
        assert!(start <= end);
        let mut start_block = start / BLOCK_SZ;
        let mut write_size = 0usize;
        loop {
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            end_current_block = end_current_block.min(end);
            let block_write_size = end_current_block - start;
            get_block_cache(
                self.get_block_id(start_block as u32, block_device) as usize,
                Arc::clone(block_device),
            )
            .lock()
            .modify(0, |data_block: &mut DataBlock| {
                let src = &buf[write_size..write_size + block_write_size];
                let dst = &mut data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_write_size];
                dst.copy_from_slice(src);
            });
            write_size += block_write_size;
            if end_current_block == end {
                break;
            }
            start_block += 1;
            start = end_current_block;
        }
        write_size
    }
}

#[repr(C)]
pub struct DirEntry {
    name: [u8; NAME_LENGTH_LIMIT + 1],
    inode_number: u32,
}

impl DirEntry {
    pub fn empty() -> Self {
        DirEntry {
            name: [0; NAME_LENGTH_LIMIT + 1],
            inode_number: 0,
        }
    }

    pub fn new(name: &str, inode_number: u32) -> Self {
        let mut bytes = [0u8; NAME_LENGTH_LIMIT + 1];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Self {
            name: bytes,
            inode_number,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const _ as usize as *const u8, DIRENT_SZ) }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self as *mut _ as usize as *mut u8, DIRENT_SZ) }
    }

    pub fn name(&self) -> &str {
        let len = (0usize..).find(|i| self.name[*i] == 0).unwrap();
        core::str::from_utf8(&self.name[..len]).unwrap()
    }

    pub fn inode_number(&self) -> u32 {
        self.inode_number
    }
}
