use crate::{
    console,
    mm::translate_byte_buffer,
    sbi::console_getchar,
    task::{processor::current_user_token, suspend_current_and_run_next},
};

const FD_STDOUT: usize = 1;
const FD_STDIN: usize = 0;

pub fn sys_write(fd: usize, buffer: *const u8, len: usize) -> isize {
    match fd {
        FD_STDOUT => {
            let buffers = translate_byte_buffer(current_user_token(), buffer, len);
            for buf in buffers {
                print!("{}", core::str::from_utf8(buf).unwrap());
            }
            len as isize
        }
        _ => panic!("Unsupported fd in sys_write!"),
    }
}

pub fn sys_read(fd: usize, buffer: *const u8, len: usize) -> isize {
    match fd {
        FD_STDIN => {
            assert_eq!(len, 1, "sys_read only support len=1!");
            let mut c: usize;
            loop {
                c = console_getchar();
                if c == 0 {
                    suspend_current_and_run_next();
                    continue;
                } else {
                    break;
                }
            }
            let ch = c as u8;
            let mut buffers = translate_byte_buffer(current_user_token(), buffer, len);
            unsafe {
                buffers[0].as_mut_ptr().write_volatile(ch);
            }
            1
        }
        _ => panic!("Unsupported fd in sys_read!"),
    }
}
