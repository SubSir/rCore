// os/src/sbi.rs
use crate::sync::UPSafeCell;
use lazy_static::*;

unsafe impl Send for MemoryManager {}
lazy_static! {
    pub static ref Memory_Managr: UPSafeCell<MemoryManager> =
        unsafe { UPSafeCell::new(MemoryManager::new()) };
}
pub fn console_putchar(c: usize) {
    unsafe {
        Memory_Managr.exclusive_access().put_char(c as u8);
    }
}

pub fn shutdown(failure: bool) -> ! {
    unreachable!()
}

const MMIO_BASE: usize = 0x10000000;
const RBR: usize = 0;
const THR: usize = 0;
const DLL: usize = 0;
const IER: usize = 1;
const DLM: usize = 1;
const IIR: usize = 2;
const FCR: usize = 2;
const LCR: usize = 3;
const MCR: usize = 4;
const LSR: usize = 5;
const MSR: usize = 6;
const SCR: usize = 7;

const IER_DISABLE_INTERRUPT: u8 = 0;
const LCR_BAUD_LATCH: u8 = 1 << 7;
const LSB_RATE: u8 = 0x03;
const MSB_RATE: u8 = 0x00;
const LCR_8BITS: u8 = 3 << 0;
const FCR_FIFO_ENABLE: u8 = 1 << 0;
const FCR_FIFO_CLEAR: u8 = 3 << 1;
const IER_TX_ENABLE: u8 = 1 << 1;
const IER_RX_ENABLE: u8 = 1 << 0;
use core::ptr;

pub struct MemoryManager {
    base_address: *mut u8,
}

impl MemoryManager {
    pub unsafe fn new() -> Self {
        let manager = Self {
            base_address: MMIO_BASE as *mut u8,
        };
        manager.init();
        manager
    }

    pub unsafe fn write_byte(&self, offset: usize, value: u8) {
        let ptr = self.base_address.add(offset);
        ptr::write_volatile(ptr, value);
    }

    pub unsafe fn read_byte(&self, offset: usize) -> u8 {
        let ptr = self.base_address.add(offset);
        ptr::read_volatile(ptr)
    }

    pub unsafe fn init(&self) {
        self.write_byte(IER, IER_DISABLE_INTERRUPT);
        self.write_byte(LCR, LCR_BAUD_LATCH);
        self.write_byte(0, LSB_RATE);
        self.write_byte(1, MSB_RATE);
        self.write_byte(LCR, LCR_8BITS);
        self.write_byte(FCR, FCR_FIFO_ENABLE | FCR_FIFO_CLEAR);
        self.write_byte(IER, IER_TX_ENABLE | IER_RX_ENABLE);
    }

    pub unsafe fn read_char(&self) -> u8 {
        while (self.read_byte(LSR) & 0x20 == 0) {}
        self.read_byte(RBR)
    }

    pub unsafe fn put_char(&self, c: u8) {
        while (self.read_byte(LSR) & 0x20 == 0) {}
        self.write_byte(THR, c);
    }
}
