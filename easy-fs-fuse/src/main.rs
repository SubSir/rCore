use clap::App;
use clap::Arg;
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
    assert!(root_node2.find("filc").is_some());
    for name in root_inode.ls() {
        println!("/ {}", name);
    }

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
    assert!(Arc::ptr_eq(&root_inode, &root_by_multics));

    // 其余内容还是原样......
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
