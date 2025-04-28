use crate::layout::DiskInodeType;
use alloc::{string::String, sync::Arc, vec::Vec};
use spin::{Mutex, MutexGuard};

use crate::{
    block_cache::get_block_cache,
    block_dev::BlockDevice,
    efs::EasyFileSystem,
    layout::{DIRENT_SZ, DirEntry, DiskInode},
};

pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    fn read_dist_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }

    fn modify_dist_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }

    pub fn find(self: &Arc<Self>, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_dist_inode(|dist_inode| {
            self.find_inode_id(name, dist_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }

    pub fn is_dir(&self) -> bool {
        self.read_dist_inode(|dist_inode| dist_inode.is_dir())
    }

    pub fn is_file(&self) -> bool {
        self.read_dist_inode(|dist_inode| dist_inode.is_file())
    }

    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device),
                DIRENT_SZ
            );
            if dirent.name() == name {
                return Some(dirent.inode_number() as u32);
            }
        }
        None
    }

    pub fn parent(&self) -> Arc<Inode> {
        let fs = self.fs.lock();
        let inode_id = self.read_dist_inode(|disk_inode| disk_inode.parent);
        let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
        Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        ))
    }

    pub fn edit_parent(&self, upper_inode: Arc<Inode>) {
        let fs = self.fs.lock();
        let inode_id = upper_inode.inode_id(&fs);
        self.modify_dist_inode(|disk_inode| disk_inode.parent = inode_id);
    }

    pub fn inode_id(&self, fs: &MutexGuard<EasyFileSystem>) -> u32 {
        fs.get_inode_id(self.block_id as u32, self.block_offset)
    }

    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_dist_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device),
                    DIRENT_SZ
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }

    pub fn create(self: &Arc<Self>, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        if self
            .read_dist_inode(|root_inode| self.find_inode_id(name, root_inode))
            .is_some()
        {
            return None;
        }
        let new_inode_id = fs.alloc_inode();
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_dist_inode(|root_inode| {
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            self.increase_size(new_size as u32, root_inode, &mut fs);
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        let new_inode = Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        ));
        drop(fs);
        new_inode.edit_parent(self.clone());
        Some(new_inode)
    }

    pub fn mkdir(self: &Arc<Self>, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        if self
            .read_dist_inode(|root_inode| {
                assert!(root_inode.is_dir());
                self.find_inode_id(name, root_inode)
            })
            .is_some()
        {
            return None;
        }
        let new_inode_id = fs.alloc_inode();
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::Directory);
            });
        self.modify_dist_inode(|root_inode| {
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            self.increase_size(new_size as u32, root_inode, &mut fs);
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        let new_inode = Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        ));
        drop(fs);
        new_inode.edit_parent(self.clone());
        Some(new_inode)
    }

    pub fn cd(self: &Arc<Self>, path: &str) -> Option<Arc<Inode>> {
        let mut inode = if path.starts_with('/') {
            Arc::new(EasyFileSystem::root_inode(&self.fs))
        } else {
            self.clone()
        };
        for token in path.split('/') {
            if token == "." || token == "" {
                continue;
            }
            if token == ".." {
                inode = inode.parent();
                continue;
            }
            inode = match inode.find(token) {
                Some(next_inode) => next_inode,
                None => return None,
            };
        }
        Some(inode)
    }

    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        self.modify_dist_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        })
    }

    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }

    fn decrease_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size > disk_inode.size {
            return;
        }
        let blocks_dealloc = disk_inode.decrease_size(new_size, &self.block_device);
        for data_block in blocks_dealloc.into_iter() {
            fs.dealloc_data(data_block);
        }
    }

    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_dist_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
    }

    pub fn remove(self: &Arc<Self>, name: &str) -> bool {
        let mut fs = self.fs.lock();
        let inode = self.read_dist_inode(|root_inode| self.find_inode_id(name, root_inode));

        if inode.is_none() {
            return false;
        }

        let (block_id, block_offset) = fs.get_disk_inode_pos(inode.unwrap());
        let erase_inode = Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        ));

        if erase_inode.is_dir() {
            if !erase_inode.read_dist_inode(|disk_inode| {
                let file_count = (disk_inode.size as usize) / DIRENT_SZ;
                if file_count != 0 {
                    return false;
                }
                return true;
            }) {
                return false;
            }
        }

        fs.dealloc_inode(inode.unwrap());

        let mut v = self.read_dist_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<DirEntry> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device),
                    DIRENT_SZ
                );
                v.push(dirent);
            }
            v
        });

        if let Some(pos) = v.iter().position(|x| x.name() == name) {
            v.remove(pos);
        }

        self.modify_dist_inode(|root_inode| {
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count - 1) * DIRENT_SZ;
            self.decrease_size(new_size as u32, root_inode, &mut fs);
            for i in 0..v.len() {
                root_inode.write_at(i * DIRENT_SZ, v[i].as_bytes(), &self.block_device);
            }
        });

        return true;
    }

    fn is_root(&self) -> bool {
        let fs = self.fs.lock();
        let inode_id = self.read_dist_inode(|disk_inode| disk_inode.parent);
        return self.inode_id(&fs) == inode_id;
    }

    pub fn mv(self: &Arc<Self>, src: &str, dst: &str) -> bool {
        let src_inode = match self.cd(src) {
            Some(inode) => inode,
            None => return false,
        };
        let parent = src_inode.parent();
        if Inode::same_inode(&parent, &src_inode) {
            return false;
        }
        if self.cd(dst).is_some() {
            return false;
        }
        let mut dst_inode = if dst.starts_with('/') {
            Arc::new(EasyFileSystem::root_inode(&self.fs))
        } else {
            self.clone()
        };
        let tokens: Vec<_> = dst.split('/').collect();
        for &token in &tokens[..tokens.len().saturating_sub(1)] {
            if token == "." || token == "" {
                continue;
            }
            if token == ".." {
                dst_inode = dst_inode.parent();
                continue;
            }
            dst_inode = match dst_inode.find(token) {
                Some(next_inode) => next_inode,
                None => return false,
            };
        }

        if dst_inode.find(tokens.last().unwrap()).is_some() {
            return false;
        }

        if Inode::is_ancestor(&src_inode, dst_inode.clone()) {
            return false;
        };

        let &last = tokens.last().unwrap();
        let tokens: Vec<_> = src.split('/').collect();
        let &name = tokens.last().unwrap();

        let mut v = parent.read_dist_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<DirEntry> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device),
                    DIRENT_SZ
                );
                v.push(dirent);
            }
            v
        });

        if let Some(pos) = v.iter().position(|x| x.name() == name) {
            v.remove(pos);
        }

        let mut fs = self.fs.lock();
        parent.modify_dist_inode(|root_inode| {
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count - 1) * DIRENT_SZ;
            parent.decrease_size(new_size as u32, root_inode, &mut fs);
            for i in 0..v.len() {
                root_inode.write_at(i * DIRENT_SZ, v[i].as_bytes(), &parent.block_device);
            }
        });

        let inode_id = src_inode.inode_id(&fs);

        dst_inode.modify_dist_inode(|root_inode| {
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            dst_inode.increase_size(new_size as u32, root_inode, &mut fs);
            let dirent = DirEntry::new(last, inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &src_inode.block_device,
            );
        });
        drop(fs);

        src_inode.edit_parent(dst_inode);

        return true;
    }

    fn is_ancestor(ancestor: &Arc<Self>, mut node: Arc<Self>) -> bool {
        loop {
            let parent = node.parent();
            if Inode::same_inode(&parent, &node) {
                break;
            }
            if Inode::same_inode(&parent, ancestor) {
                return true;
            }
            node = parent;
        }
        false
    }

    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_dist_inode(|dist_inode| dist_inode.read_at(offset, buf, &self.block_device))
    }

    pub fn same_inode(node1: &Arc<Self>, node2: &Arc<Self>) -> bool {
        node1.block_id == node2.block_id && node1.block_offset == node2.block_offset
    }
}
