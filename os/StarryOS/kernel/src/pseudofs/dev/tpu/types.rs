//! TPU 数据结构定义

pub const IOCTL_TPU_BASE: u8 = b'p';

const fn iow(ty: u8, nr: u8, size: usize) -> u32 {
    (1u32 << 30) | ((size as u32) << 16) | ((ty as u32) << 8) | (nr as u32)
}

const fn iowr(ty: u8, nr: u8, size: usize) -> u32 {
    (3u32 << 30) | ((size as u32) << 16) | ((ty as u32) << 8) | (nr as u32)
}

pub const CVITPU_SUBMIT_DMABUF: u32 = iow(IOCTL_TPU_BASE, 0x01, 8);
pub const CVITPU_DMABUF_FLUSH_FD: u32 = iow(IOCTL_TPU_BASE, 0x02, 8);
pub const CVITPU_DMABUF_INVLD_FD: u32 = iow(IOCTL_TPU_BASE, 0x03, 8);
pub const CVITPU_DMABUF_FLUSH: u32 = iow(IOCTL_TPU_BASE, 0x04, 8);
pub const CVITPU_DMABUF_INVLD: u32 = iow(IOCTL_TPU_BASE, 0x05, 8);
pub const CVITPU_WAIT_DMABUF: u32 = iowr(IOCTL_TPU_BASE, 0x06, 8);
pub const CVITPU_PIO_MODE: u32 = iow(IOCTL_TPU_BASE, 0x07, 8);
pub const CVITPU_LOAD_TEE: u32 = iowr(IOCTL_TPU_BASE, 0x08, 8);
pub const CVITPU_SUBMIT_TEE: u32 = iow(IOCTL_TPU_BASE, 0x09, 8);
pub const CVITPU_UNLOAD_TEE: u32 = iow(IOCTL_TPU_BASE, 0x0A, 8);
pub const CVITPU_SUBMIT_PIO: u32 = iow(IOCTL_TPU_BASE, 0x0B, 8);
pub const CVITPU_WAIT_PIO: u32 = iowr(IOCTL_TPU_BASE, 0x0C, 8);

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CviCacheOpArg {
    pub paddr: u64,
    pub size: u64,
    pub dma_fd: i32,
    pub _padding: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CviSubmitDmaArg {
    pub fd: i32,
    pub seq_no: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CviWaitDmaArg {
    pub seq_no: u32,
    pub ret: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CviPioMode {
    pub cmdbuf: u64,
    pub sz: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CviLoadTeeArg {
    pub cmdbuf_addr_ree: u64,
    pub cmdbuf_len_ree: u32,
    pub _pad1: u32,
    pub weight_addr_ree: u64,
    pub weight_len_ree: u32,
    pub _pad2: u32,
    pub neuron_addr_ree: u64,
    pub dmabuf_addr_tee: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CviSubmitTeeArg {
    pub dmabuf_tee_addr: u64,
    pub gaddr_base2: u64,
    pub gaddr_base3: u64,
    pub gaddr_base4: u64,
    pub gaddr_base5: u64,
    pub gaddr_base6: u64,
    pub gaddr_base7: u64,
    pub seq_no: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CviUnloadTeeArg {
    pub addr: u64,
    pub size: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CviTdmaCopyArg {
    pub paddr_src: u64,
    pub paddr_dst: u64,
    pub h: u32,
    pub w_bytes: u32,
    pub stride_bytes_src: u32,
    pub stride_bytes_dst: u32,
    pub enable_2d: u32,
    pub leng_bytes: u32,
    pub seq_no: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CviTdmaWaitArg {
    pub seq_no: u32,
    pub ret: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DmaHeader {
    pub dmabuf_magic_m: u16,
    pub dmabuf_magic_s: u16,
    pub dmabuf_size: u32,
    pub cpu_desc_count: u32,
    pub bd_desc_count: u32,
    pub tdma_desc_count: u32,
    pub tpu_clk_rate: u32,
    pub pmubuf_size: u32,
    pub pmubuf_offset: u32,
    pub arraybase_0_l: u32,
    pub arraybase_0_h: u32,
    pub arraybase_1_l: u32,
    pub arraybase_1_h: u32,
    pub arraybase_2_l: u32,
    pub arraybase_2_h: u32,
    pub arraybase_3_l: u32,
    pub arraybase_3_h: u32,
    pub arraybase_4_l: u32,
    pub arraybase_4_h: u32,
    pub arraybase_5_l: u32,
    pub arraybase_5_h: u32,
    pub arraybase_6_l: u32,
    pub arraybase_6_h: u32,
    pub arraybase_7_l: u32,
    pub arraybase_7_h: u32,
    pub reserved: [u32; 8],
}

impl DmaHeader {
    pub fn is_valid(&self) -> bool { self.dmabuf_magic_m == super::TPU_DMABUF_HEADER_M }

    pub fn has_valid_pmu(&self) -> bool {
        self.pmubuf_offset != 0
            && self.pmubuf_size != 0
            && (self.pmubuf_offset & 0xF) == 0
            && (self.pmubuf_size & 0xF) == 0
    }
}

pub const CPU_ENGINE_DESCRIPTOR_NUM: usize = 56;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CpuSyncDesc {
    pub op_type: u32,
    pub num_bd: u32,
    pub num_gdma: u32,
    pub offset_bd: u32,
    pub offset_gdma: u32,
    pub reserved: [u32; 2],
    pub str_data: [u8; (CPU_ENGINE_DESCRIPTOR_NUM - 7) * 4],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CmdIdNode {
    pub bd_cmd_id: u32,
    pub tdma_cmd_id: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct TpuPlatformCfg {
    pub tdma_base: *mut u8,
    pub tiu_base: *mut u8,
    pub pmubuf_addr_p: u64,
    pub pmubuf_size: u32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpuPmuEvent {
    BankConflict    = 0x0,
    StallCount      = 0x1,
    TdmaBandwidth   = 0x2,
    TdmaWriteStrobe = 0x3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TdmaReg {
    pub vld: u32, pub compress_en: u32, pub eod: u32, pub intp_en: u32,
    pub bar_en: u32, pub check_bf16_value: u32, pub trans_dir: u32, pub rsv00: u32,
    pub trans_fmt: u32, pub transpose_md: u32, pub rsv01: u32, pub intra_cmd_paral: u32,
    pub outstanding_en: u32, pub cmd_id: u32, pub spec_func: u32, pub dst_fmt: u32,
    pub src_fmt: u32, pub cmprs_fmt: u32, pub sys_dtype: u32, pub rsv2_1: u32,
    pub int8_sign: u32, pub compress_zero_guard: u32, pub int8_rnd_mode: u32,
    pub wait_id_tpu: u32, pub wait_id_other_tdma: u32, pub wait_id_sdma: u32,
    pub const_val: u32, pub src_base_reg_sel: u32, pub mv_lut_idx: u32,
    pub dst_base_reg_sel: u32, pub mv_lut_base: u32, pub rsv4_5: u32,
    pub dst_h_stride: u32, pub dst_c_stride_low: u32, pub dst_n_stride: u32,
    pub src_h_stride: u32, pub src_c_stride_low: u32, pub src_n_stride: u32,
    pub dst_c: u32, pub src_c: u32, pub dst_w: u32, pub dst_h: u32,
    pub src_w: u32, pub src_h: u32, pub dst_base_addr_low: u32, pub src_base_addr_low: u32,
    pub src_n: u32, pub dst_base_addr_high: u32, pub src_base_addr_high: u32,
    pub src_c_stride_high: u32, pub dst_c_stride_high: u32, pub compress_bias0: u32,
    pub compress_bias1: u32, pub layer_id: u32,
}
