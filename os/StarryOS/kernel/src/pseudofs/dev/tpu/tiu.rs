//! TIU (Tensor Instruction Unit) 寄存器定义和操作

use core::ptr::{read_volatile, write_volatile};

pub const BDC_ENGINE_CMD_ALIGNED_BIT: u32 = 8;
pub const BD_CTRL_BASE_ADDR: usize = 0x100;
pub const BD_TPU_EN: u32 = 0;
pub const BD_DES_ADDR_VLD: u32 = 30;
pub const BD_INTR_ENABLE: u32 = 31;

pub struct TiuRegs {
    base: *mut u8,
}

unsafe impl Sync for TiuRegs {}
unsafe impl Send for TiuRegs {}

impl TiuRegs {
    pub const unsafe fn new(base: *mut u8) -> Self { Self { base } }

    #[inline]
    pub fn read(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base as usize + offset) as *const u32) }
    }

    #[inline]
    pub fn write(&self, offset: usize, value: u32) {
        unsafe { write_volatile((self.base as usize + offset) as *mut u32, value) }
    }

    pub fn base(&self) -> *mut u8 { self.base }

    #[inline]
    pub fn read_bd_ctrl(&self, offset: usize) -> u32 { self.read(BD_CTRL_BASE_ADDR + offset) }

    #[inline]
    pub fn write_bd_ctrl(&self, offset: usize, value: u32) {
        self.write(BD_CTRL_BASE_ADDR + offset, value)
    }

    pub fn get_current_bd_id(&self) -> u32 { (self.read_bd_ctrl(0) >> 6) & 0xFFFF }

    pub fn is_bd_interrupt(&self) -> bool { (self.read_bd_ctrl(0) & (1 << 1)) != 0 }

    pub fn clear_bd_interrupt(&self) {
        let reg_val = self.read_bd_ctrl(0);
        self.write_bd_ctrl(0, reg_val | (1 << 1));
    }

    pub fn reset_id(&self) {
        let reg_val = self.read_bd_ctrl(0xC);
        self.write_bd_ctrl(0xC, reg_val | 0x1);
        self.write_bd_ctrl(0xC, reg_val & !0x1);
        let reg_val = self.read_bd_ctrl(0);
        self.write_bd_ctrl(0, reg_val & !((1 << BD_TPU_EN) | (1 << BD_DES_ADDR_VLD)));
        let reg_val = self.read_bd_ctrl(0);
        self.write_bd_ctrl(0, reg_val | (1 << 1));
    }

    pub fn fire_descriptor(&self, desc_offset: u64, _num_bd: u32) {
        let desc_addr = desc_offset << BDC_ENGINE_CMD_ALIGNED_BIT;
        self.write_bd_ctrl(0x4, (desc_addr & 0xFFFFFFFF) as u32);
        let reg_val = self.read_bd_ctrl(0x8);
        self.write_bd_ctrl(0x8, (reg_val & 0xFFFFFF00) | ((desc_addr >> 32) as u32 & 0xFF));
        let reg_val = self.read_bd_ctrl(0xC);
        self.write_bd_ctrl(0xC, reg_val | (1 << 11));
        let reg_val = self.read_bd_ctrl(0);
        let reg_val = reg_val & !0x3FC00000;
        self.write_bd_ctrl(0, reg_val | (3 << 22));
        let reg_val = self.read_bd_ctrl(0);
        self.write_bd_ctrl(0, reg_val
            | (1 << BD_DES_ADDR_VLD)
            | (1 << BD_INTR_ENABLE)
            | (1 << BD_TPU_EN));
    }
}
