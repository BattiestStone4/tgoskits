//! TDMA 寄存器定义和操作

use core::ptr::{read_volatile, write_volatile};

pub const TDMA_DESC_REG_BYTES: usize = 0x40;
pub const TDMA_ENGINE_DESCRIPTOR_NUM: usize = TDMA_DESC_REG_BYTES >> 2;
pub const TDMA_NUM_BASE_REGS: usize = 0x8;

pub const TDMA_CTRL: usize = 0x0;
pub const TDMA_DES_BASE: usize = 0x4;
pub const TDMA_INT_MASK: usize = 0x8;
pub const TDMA_SYNC_STATUS: usize = 0xC;
pub const TDMA_ARRAYBASE0_L: usize = 0x70;
pub const TDMA_ARRAYBASE1_L: usize = 0x74;
pub const TDMA_ARRAYBASE2_L: usize = 0x78;
pub const TDMA_ARRAYBASE3_L: usize = 0x7C;
pub const TDMA_ARRAYBASE4_L: usize = 0x80;
pub const TDMA_ARRAYBASE5_L: usize = 0x84;
pub const TDMA_ARRAYBASE6_L: usize = 0x88;
pub const TDMA_ARRAYBASE7_L: usize = 0x8C;
pub const TDMA_ARRAYBASE0_H: usize = 0x90;
pub const TDMA_ARRAYBASE1_H: usize = 0x94;
pub const TDMA_DEBUG_MODE: usize = 0xA0;
pub const TDMA_DCM_DISABLE: usize = 0xA4;
pub const TDMA_STATUS: usize = 0xEC;
pub const TPUPMU_CTRL: usize = 0x200;
pub const TPUPMU_BUFBASE: usize = 0x20C;
pub const TPUPMU_BUFSIZE: usize = 0x210;

pub const TDMA_CTRL_ENABLE_BIT: u32 = 0;
pub const TDMA_CTRL_MODESEL_BIT: u32 = 1;
pub const TDMA_CTRL_RESET_SYNCID_BIT: u32 = 2;
pub const TDMA_CTRL_FORCE_1ARRAY: u32 = 5;
pub const TDMA_CTRL_FORCE_2ARRAY: u32 = 6;
pub const TDMA_CTRL_BURSTLEN_BIT: u32 = 8;
pub const TDMA_CTRL_64BYTE_ALIGN_EN: u32 = 10;
pub const TDMA_CTRL_INTRA_CMD_OFF: u32 = 13;
pub const TDMA_CTRL_DESNUM_BIT: u32 = 16;

pub const TDMA_MASK_INIT: u32 = 0x20;
pub const TDMA_INT_EOD: u32 = 0x1;
pub const TDMA_INT_EOPMU: u32 = 0x8000;
pub const TDMA_ALL_IDLE: u32 = 0x1F;

pub struct TdmaRegs {
    base: *mut u8,
}

unsafe impl Sync for TdmaRegs {}
unsafe impl Send for TdmaRegs {}

impl TdmaRegs {
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

    pub fn get_int_status(&self) -> u32 {
        let reg_value = self.read(TDMA_INT_MASK);
        (reg_value >> 16) & !TDMA_MASK_INIT
    }

    pub fn clear_interrupt(&self) { self.write(TDMA_INT_MASK, 0xFFFF0000); }

    pub fn get_sync_tdma_id(&self) -> u32 { self.read(TDMA_SYNC_STATUS) >> 16 }

    pub fn set_array_bases(&self, header: &super::types::DmaHeader) {
        self.write(TDMA_ARRAYBASE0_L, header.arraybase_0_l);
        self.write(TDMA_ARRAYBASE1_L, header.arraybase_1_l);
        self.write(TDMA_ARRAYBASE2_L, header.arraybase_2_l);
        self.write(TDMA_ARRAYBASE3_L, header.arraybase_3_l);
        self.write(TDMA_ARRAYBASE4_L, header.arraybase_4_l);
        self.write(TDMA_ARRAYBASE5_L, header.arraybase_5_l);
        self.write(TDMA_ARRAYBASE6_L, header.arraybase_6_l);
        self.write(TDMA_ARRAYBASE7_L, header.arraybase_7_l);
        self.write(TDMA_ARRAYBASE0_H, 0);
        self.write(TDMA_ARRAYBASE1_H, 0);
    }

    pub fn reset_sync_id(&self) {
        self.write(TDMA_CTRL, 1 << TDMA_CTRL_RESET_SYNCID_BIT);
        self.write(TDMA_CTRL, 0);
        self.write(TDMA_INT_MASK, 0xFFFF0000);
    }

    pub fn fire_descriptor(&self, desc_offset: u64, num_tdma: u32) {
        self.write(TDMA_DES_BASE, desc_offset as u32);
        self.write(TDMA_DEBUG_MODE, 0);
        self.write(TDMA_DCM_DISABLE, 0);
        self.write(TDMA_INT_MASK, TDMA_MASK_INIT);
        let ctrl = (1 << TDMA_CTRL_ENABLE_BIT)
            | (1 << TDMA_CTRL_MODESEL_BIT)
            | (num_tdma << TDMA_CTRL_DESNUM_BIT)
            | (0x3 << TDMA_CTRL_BURSTLEN_BIT)
            | (1 << TDMA_CTRL_FORCE_1ARRAY)
            | (1 << TDMA_CTRL_INTRA_CMD_OFF)
            | (1 << TDMA_CTRL_64BYTE_ALIGN_EN);
        self.write(TDMA_CTRL, ctrl);
    }
}
