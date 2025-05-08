use clap::App;
use clap::Arg;
use easy_fs::Inode;
use easy_fs::{BlockDevice, EasyFileSystem};
use std::fs::File;
use std::fs::OpenOptions;
use std::fs::read_dir;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Arc;
const BLOCK_SZ: usize = 512;
use std::sync::Mutex;

struct BlockFile(Mutex<File>);

impl BlockDevice for BlockFile {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SZ) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.read(buf).unwrap(), BLOCK_SZ, "Not a complete block!");
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SZ) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.write(buf).unwrap(), BLOCK_SZ, "Not a complete block!");
    }
}

#[test]
fn efs_test() -> std::io::Result<()> {
    let block_file = Arc::new(BlockFile(Mutex::new({
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("target/fs.img")?;
        f.set_len(8192 * 512).unwrap();
        f
    })));
    EasyFileSystem::create(block_file.clone(), 4096, 1);
    let efs = EasyFileSystem::open(block_file.clone());
    let root_inode = Arc::new(EasyFileSystem::root_inode(&efs));
    root_inode.create("filea");
    root_inode.create("fileb");
    root_inode.create("filec");

    // 增强remove测试
    assert!(root_inode.remove("filec"), "remove filec should succeed");
    assert!(
        !root_inode.remove("filec"),
        "removing the same file again should fail"
    );
    assert!(root_inode.remove("filea"), "remove filea should succeed");
    assert!(
        !root_inode.remove("no_such_file"),
        "removing nonexistent file should fail"
    );
    println!("remove filea and filec");

    for name in root_inode.ls() {
        println!("/ {}", name);
    }
    assert!(root_inode.find("fileb").is_some());
    assert!(root_inode.find("filea").is_none());
    assert!(root_inode.find("filec").is_none());

    root_inode.create("filea");
    root_inode.mkdir("dir");
    // 增强mkdir测试
    assert!(
        root_inode.mkdir("dir").is_none(),
        "making same directory should fail"
    );
    println!("make filea and dir");

    for name in root_inode.ls() {
        println!("/ {}", name);
    }

    let dir = root_inode.cd("dir").unwrap();
    dir.create("dira");
    dir.mkdir("subdir1");
    dir.mkdir("subdir2");
    assert!(dir.cd("subdir1").is_some());
    assert!(dir.cd("subdir2").is_some());
    assert!(dir.cd("no_such_dir").is_none());

    // 多层级mkdir和cd
    let subdir1 = dir.cd("subdir1").unwrap();
    subdir1.mkdir("nested");
    let nested = subdir1.cd("nested").unwrap();
    nested.create("deepfile");
    assert!(nested.find("deepfile").is_some());

    // 目录ls输出覆盖
    for name in dir.ls() {
        println!("/dir/ {}", name);
    }
    for name in subdir1.ls() {
        println!("/dir/subdir1/ {}", name);
    }
    for name in nested.ls() {
        println!("/dir/subdir1/nested/ {}", name);
    }

    // cd .. 测试
    let root_node2 = dir.cd("../").unwrap();
    root_node2.mkdir("filc");
    for name in root_node2.ls() {
        println!("/root_node2/ {}", name);
    }
    for name in root_inode.ls() {
        println!("/ {}", name);
    }
    assert!(root_inode.find("filc").is_some());

    // remove目录/空目录/非空目录
    assert!(dir.remove("dira"), "remove file in dir should succeed");
    for name in dir.ls() {
        println!("/ {}", name);
    }
    assert!(dir.remove("subdir2"), "remove subdir2 should succeed");
    assert!(
        !dir.remove("subdir1"),
        "should not remove non-empty subdir1"
    ); // 先不为空
    // 清空subdir1
    let subdir1 = dir.cd("subdir1").unwrap();
    let nested = subdir1.cd("nested").unwrap();
    assert!(nested.remove("deepfile"), "remove nested file");
    assert!(
        !nested.remove("deepfile"),
        "remove non existent file again fail"
    );
    // 删掉nested目录
    assert!(subdir1.remove("nested"), "remove empty nested dir");
    assert!(dir.remove("subdir1"), "subdir1 should now be removed");
    assert!(!dir.remove("subdir1"), "subdir1 already gone");

    // cd嵌套和..多次
    let root_by_multics = dir.cd("../../").unwrap();
    for name in root_by_multics.ls() {
        println!("/root_by_multics/ {}", name);
    }

    for name in root_inode.ls() {
        println!("/root_inode/ {}", name);
    }

    assert!(Inode::same_inode(&root_inode, &root_by_multics));

    // ===== Inode mv 方法测试 Start =====
    println!("--- mv method test ---");
    // 1. 简单重命名文件
    root_inode.create("mv_file1");

    assert!(
        root_inode.mv("mv_file1", "mv_file2"),
        "mv rename file should succeed"
    );

    assert!(
        root_inode.find("mv_file1").is_none(),
        "old name should not exist after mv"
    );
    assert!(
        root_inode.find("mv_file2").is_some(),
        "new name should exist after mv"
    );

    // 2. mv 不存在文件，应该失败
    assert!(
        !root_inode.mv("no_such_file", "mv_file3"),
        "mv non-existent file should fail"
    );

    // 3. 目标名字已存在，mv 应该失败（假定你的实现这样设计，如果目标若已存在不得覆盖）
    root_inode.create("mv_file3");
    assert!(
        !root_inode.mv("mv_file3", "mv_file2"),
        "mv to existing filename should fail"
    );

    // 4. 目录重命名
    root_inode.mkdir("old_dir");
    assert!(
        root_inode.mv("old_dir", "new_dir"),
        "mv dir rename should succeed"
    );
    assert!(root_inode.find("old_dir").is_none());
    assert!(root_inode.find("new_dir").is_some());

    // 5. 子目录内部移动
    let new_dir = root_inode.cd("new_dir").unwrap();
    new_dir.create("inner_file");
    assert!(
        root_inode.mv("new_dir/inner_file", "mv_inner_file"),
        "mv file from subdir to root"
    );
    assert!(root_inode.find("mv_inner_file").is_some());
    assert!(new_dir.find("inner_file").is_none());

    // 6. 跨目录移动：重命名到子目录下
    root_inode.create("mv_file4");
    assert!(
        new_dir.mv("../mv_file4", "movedfile"),
        "mv跨目录到当前目录应该成功"
    );
    assert!(new_dir.find("movedfile").is_some());
    assert!(root_inode.find("mv_file4").is_none());

    // 7. 不能移动目录到自身子目录（如果你有实现相关保护）
    root_inode.mkdir("mv_parent");
    let mv_parent = root_inode.cd("mv_parent").unwrap();
    println!("mv_parent:");
    mv_parent.mkdir("child_dir");
    assert!(
        !root_inode.mv("mv_parent", "mv_parent/child_dir/newname"),
        "mv parent to its child should fail"
    );

    // 8. 目录重命名后, 能正常用
    assert!(root_inode.mv("mv_parent", "mv_parent2"));
    assert!(root_inode.find("mv_parent2").is_some());
    assert!(
        root_inode
            .cd("mv_parent2")
            .unwrap()
            .find("child_dir")
            .is_some()
    );

    root_inode.mkdir("tmpa");
    root_inode.mkdir("tmpb");
    assert!(root_inode.mv("tmpa", "tmpb/"));

    // 输出最终结构便于人工确认
    println!("final root_inode ls:");
    for name in root_inode.ls() {
        println!("/ {}", name);
    }
    // ===== Inode mv 方法测试 End =====

    let filea = root_inode.find("filea").unwrap();
    let greet_str = "Hello, world!";
    filea.write_at(0, greet_str.as_bytes());
    let mut buffer = [0u8; 233];
    let len = filea.read_at(0, &mut buffer);
    assert_eq!(greet_str, core::str::from_utf8(&buffer[..len]).unwrap());

    let mut random_str_test = |len: usize| {
        filea.clear();
        assert_eq!(filea.read_at(0, &mut buffer), 0);
        let mut str = String::new();
        use rand;
        for _ in 0..len {
            str.push(char::from('0' as u8 + rand::random::<u8>() % 10));
        }
        filea.write_at(0, str.as_bytes());
        let mut read_buffer = [0u8; 127];
        let mut offset = 0usize;
        let mut read_str = String::new();
        loop {
            let len = filea.read_at(offset, &mut read_buffer);
            if len == 0 {
                break;
            }
            offset += len;
            read_str.push_str(core::str::from_utf8(&read_buffer[..len]).unwrap());
        }
        assert_eq!(str, read_str);
    };

    random_str_test(4 * BLOCK_SZ);
    random_str_test(8 * BLOCK_SZ + BLOCK_SZ / 2);
    random_str_test(100 * BLOCK_SZ);
    random_str_test(70 * BLOCK_SZ + BLOCK_SZ / 7);
    random_str_test((12 + 128) * BLOCK_SZ);
    random_str_test(400 * BLOCK_SZ);
    random_str_test(1000 * BLOCK_SZ);
    random_str_test(2000 * BLOCK_SZ);

    Ok(())
}
fn easy_fs_pack() -> std::io::Result<()> {
    let matches = App::new("EasyFileSystem packer")
        .arg(
            Arg::with_name("source")
                .short("s")
                .long("source")
                .takes_value(true)
                .help("Executable source dir(with backslash)"),
        )
        .arg(
            Arg::with_name("target")
                .short("t")
                .long("target")
                .takes_value(true)
                .help("Executable target dir(with backslash)"),
        )
        .get_matches();
    let src_path = matches.value_of("source").unwrap();
    let target_path = matches.value_of("target").unwrap();
    println!("src_path: {}, target_path: {}", src_path, target_path);
    let block_file = Arc::new(BlockFile(Mutex::new({
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(format!("{}{}", target_path, "fs.img"))?;
        f.set_len(16 * 2048 * 512).unwrap();
        f
    })));
    let efs = EasyFileSystem::create(block_file, 16 * 2048, 1);
    let root_inode = Arc::new(EasyFileSystem::root_inode(&efs));
    let apps: Vec<_> = read_dir(src_path)
        .unwrap()
        .into_iter()
        .map(|dir_entry| {
            let mut name_with_ext = dir_entry.unwrap().file_name().into_string().unwrap();
            name_with_ext.drain(name_with_ext.find('.').unwrap()..name_with_ext.len());
            name_with_ext
        })
        .collect();
    for app in apps {
        let mut host_file = File::open(format!("{}{}", target_path, app)).unwrap();
        let mut all_data = Vec::<u8>::new();
        host_file.read_to_end(&mut all_data).unwrap();
        let inode = root_inode.create(&app.as_str()).unwrap();
        inode.write_at(0, all_data.as_slice());
    }
    for app in root_inode.ls() {
        println!("{}", app);
    }
    Ok(())
}

fn main() {
    easy_fs_pack().expect("Error when packing easy-fs!");
}
