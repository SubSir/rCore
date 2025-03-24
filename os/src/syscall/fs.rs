use crate::mm::translate_byte_buffer;
use crate::task::current_user_token;

const FD_STDOUT: usize = 1;

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
