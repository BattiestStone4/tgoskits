//! TPU 平台操作

use super::error::TpuError;
use super::tdma::{TdmaRegs, TPUPMU_CTRL, TPUPMU_BUFBASE, TPUPMU_BUFSIZE};
use super::tiu::TiuRegs;
use super::types::{DmaHeader, CpuSyncDesc, CmdIdNode, TpuPmuEvent};

#[derive(Debug, Clone, Copy, Default)]
pub struct TpuRegBackup {
    pub tdma_int_mask: u32,
    pub tdma_sync_status: u32,
    pub tiu_ctrl_base_address: u32,
    pub tdma_arraybase0_l: u32,
    pub tdma_arraybase1_l: u32,
    pub tdma_arraybase2_l: u32,
    pub tdma_arraybase3_l: u32,
    pub tdma_arraybase4_l: u32,
    pub tdma_arraybase5_l: u32,
    pub tdma_arraybase6_l: u32,
    pub tdma_arraybase7_l: u32,
    pub tdma_arraybase0_h: u32,
    pub tdma_arraybase1_h: u32,
    pub tdma_des_base: u32,
    pub tdma_dbg_mode: u32,
    pub tdma_dcm_disable: u32,
    pub tdma_ctrl: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TpuRuntimeState {
    pub irq_received: bool,
    pub reg_backup: TpuRegBackup,
}

pub fn pmu_enable(tdma: &TdmaRegs, pmubuf_addr_p: u64, pmubuf_size: u32, event: TpuPmuEvent) {
    let buf_addr = pmubuf_addr_p >> 4;
    let buf_size = (pmubuf_size as u64) >> 4;
    tdma.write(TPUPMU_BUFBASE, buf_addr as u32);
    tdma.write(TPUPMU_BUFSIZE, buf_size as u32);
    let mut reg_value: u32 = 0;
    reg_value |= 0x1;
    reg_value |= 0x8;
    reg_value |= 0x10;
    reg_value |= (event as u32) << 5;
    reg_value |= 0x3 << 8;
    reg_value |= 0x1 << 10;
    reg_value &= !0xFFFF0000;
    tdma.write(TPUPMU_CTRL, reg_value);
}

pub fn pmu_disable(tdma: &TdmaRegs) {
    let reg_value = tdma.read(TPUPMU_CTRL);
    tdma.write(TPUPMU_CTRL, reg_value & !0x1);
}

pub fn resync_cmd_id(tdma: &TdmaRegs, tiu: &TiuRegs) {
    tiu.reset_id();
    tdma.reset_sync_id();
}

pub fn handle_tdma_irq(tdma: &TdmaRegs, tiu: &TiuRegs, state: &mut TpuRuntimeState) -> bool {
    let reg_value = tdma.read(super::tdma::TDMA_INT_MASK);
    let int_status = (reg_value >> 16) & !super::tdma::TDMA_MASK_INIT;
    let has_error = int_status != super::tdma::TDMA_INT_EOD
        && int_status != super::tdma::TDMA_INT_EOPMU;
    tdma.clear_interrupt();
    state.reg_backup.tdma_int_mask = tdma.read(super::tdma::TDMA_INT_MASK);
    state.reg_backup.tdma_sync_status = tdma.read(super::tdma::TDMA_SYNC_STATUS);
    state.reg_backup.tiu_ctrl_base_address = tiu.read_bd_ctrl(0);
    state.irq_received = true;
    has_error
}

pub fn poll_cmdbuf_done(
    tiu: &TiuRegs,
    id_node: &CmdIdNode,
    state: &TpuRuntimeState,
    timeout_checker: impl Fn() -> bool,
) -> Result<(), TpuError> {
    if id_node.tdma_cmd_id > 0 {
        let _tdma_id = state.reg_backup.tdma_sync_status >> 16;
    }
    if id_node.bd_cmd_id > 0 {
        loop {
            let reg_val = tiu.read_bd_ctrl(0);
            let current_id = (reg_val >> 6) & 0xFFFF;
            let int_flag = (reg_val & (1 << 1)) != 0;
            if current_id >= id_node.bd_cmd_id && int_flag {
                tiu.write_bd_ctrl(0, reg_val | (1 << 1));
                break;
            }
            if timeout_checker() { return Err(TpuError::Timeout); }
            core::hint::spin_loop();
        }
    }
    Ok(())
}

pub fn run_dmabuf(
    tdma: &TdmaRegs,
    tiu: &TiuRegs,
    dmabuf_vaddr: *const u8,
    dmabuf_paddr: u64,
    state: &mut TpuRuntimeState,
    wait_irq: impl Fn() -> Result<(), TpuError>,
    timeout_checker: impl Fn() -> bool,
) -> Result<(), TpuError> {
    let header = unsafe { &*(dmabuf_vaddr as *const DmaHeader) };
    if !header.is_valid() { return Err(TpuError::InvalidDmabuf); }
    if (dmabuf_paddr & 0xFFF) != 0 { return Err(TpuError::DmabufNotAligned); }
    state.irq_received = false;
    tdma.set_array_bases(header);
    let pmu_enabled = header.has_valid_pmu();
    let pmubuf_addr = dmabuf_paddr + header.pmubuf_offset as u64;
    if pmu_enabled {
        pmu_enable(tdma, pmubuf_addr, header.pmubuf_size, TpuPmuEvent::TdmaBandwidth);
    }
    let desc_base = unsafe {
        dmabuf_vaddr.add(core::mem::size_of::<DmaHeader>()) as *const CpuSyncDesc
    };
    for i in 0..header.cpu_desc_count {
        let desc = unsafe { &*desc_base.add(i as usize) };
        let bd_num = (desc.num_bd & 0xFFFF) as u32;
        let tdma_num = (desc.num_gdma & 0xFFFF) as u32;
        let bd_offset = desc.offset_bd;
        let tdma_offset = desc.offset_gdma;
        resync_cmd_id(tdma, tiu);
        state.irq_received = false;
        let id_node = CmdIdNode { bd_cmd_id: bd_num, tdma_cmd_id: tdma_num };
        if bd_num > 0 { tiu.fire_descriptor(bd_offset as u64, bd_num); }
        if tdma_num > 0 { tdma.fire_descriptor(tdma_offset as u64, tdma_num); }
        if tdma_num > 0 { wait_irq()?; }
        poll_cmdbuf_done(tiu, &id_node, state, &timeout_checker)?;
    }
    if pmu_enabled {
        state.irq_received = false;
        pmu_disable(tdma);
        wait_irq()?;
    }
    Ok(())
}

pub fn backup_registers(tdma: &TdmaRegs, tiu: &TiuRegs, backup: &mut TpuRegBackup) {
    use super::tdma::*;
    backup.tdma_int_mask = tdma.read(TDMA_INT_MASK);
    backup.tdma_sync_status = tdma.read(TDMA_SYNC_STATUS);
    backup.tiu_ctrl_base_address = tiu.read_bd_ctrl(0);
    backup.tdma_arraybase0_l = tdma.read(TDMA_ARRAYBASE0_L);
    backup.tdma_arraybase1_l = tdma.read(TDMA_ARRAYBASE1_L);
    backup.tdma_arraybase2_l = tdma.read(TDMA_ARRAYBASE2_L);
    backup.tdma_arraybase3_l = tdma.read(TDMA_ARRAYBASE3_L);
    backup.tdma_arraybase4_l = tdma.read(TDMA_ARRAYBASE4_L);
    backup.tdma_arraybase5_l = tdma.read(TDMA_ARRAYBASE5_L);
    backup.tdma_arraybase6_l = tdma.read(TDMA_ARRAYBASE6_L);
    backup.tdma_arraybase7_l = tdma.read(TDMA_ARRAYBASE7_L);
    backup.tdma_arraybase0_h = tdma.read(TDMA_ARRAYBASE0_H);
    backup.tdma_arraybase1_h = tdma.read(TDMA_ARRAYBASE1_H);
    backup.tdma_des_base = tdma.read(TDMA_DES_BASE);
    backup.tdma_dbg_mode = tdma.read(TDMA_DEBUG_MODE);
    backup.tdma_dcm_disable = tdma.read(TDMA_DCM_DISABLE);
    backup.tdma_ctrl = tdma.read(TDMA_CTRL);
}

pub fn restore_registers(tdma: &TdmaRegs, tiu: &TiuRegs, backup: &TpuRegBackup) {
    use super::tdma::*;
    tdma.write(TDMA_INT_MASK, backup.tdma_int_mask);
    tdma.write(TDMA_SYNC_STATUS, backup.tdma_sync_status);
    tiu.write_bd_ctrl(0, backup.tiu_ctrl_base_address);
    tdma.write(TDMA_ARRAYBASE0_L, backup.tdma_arraybase0_l);
    tdma.write(TDMA_ARRAYBASE1_L, backup.tdma_arraybase1_l);
    tdma.write(TDMA_ARRAYBASE2_L, backup.tdma_arraybase2_l);
    tdma.write(TDMA_ARRAYBASE3_L, backup.tdma_arraybase3_l);
    tdma.write(TDMA_ARRAYBASE4_L, backup.tdma_arraybase4_l);
    tdma.write(TDMA_ARRAYBASE5_L, backup.tdma_arraybase5_l);
    tdma.write(TDMA_ARRAYBASE6_L, backup.tdma_arraybase6_l);
    tdma.write(TDMA_ARRAYBASE7_L, backup.tdma_arraybase7_l);
    tdma.write(TDMA_ARRAYBASE0_H, backup.tdma_arraybase0_h);
    tdma.write(TDMA_ARRAYBASE1_H, backup.tdma_arraybase1_h);
    tdma.write(TDMA_DES_BASE, backup.tdma_des_base);
    tdma.write(TDMA_DEBUG_MODE, backup.tdma_dbg_mode);
    tdma.write(TDMA_DCM_DISABLE, backup.tdma_dcm_disable);
}
