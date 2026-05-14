//! TPU 设备抽象

use alloc::{collections::VecDeque, sync::Arc};
use core::cell::Cell;
use core::sync::atomic::{AtomicU32, Ordering};

use ax_sync::Mutex;

use super::{
    TDMA_PHYS_BASE, TIU_PHYS_BASE, error::TpuError, platform::TpuRuntimeState, tdma::TdmaRegs,
    tiu::TiuRegs, types::*,
};
use crate::file::{get_file_like, ion::IonBufferFile};
use crate::pseudofs::{
    DeviceOps,
    dev::ion::{global_ion_buffer_manager, IonBufferManager, IonHandle},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpuState { Uninitialized, Idle, Running, Suspended }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpuSubmitPath { DesNormal = 0 }

#[derive(Debug)]
pub struct TpuTaskNode {
    pub pid: u32,
    pub seq_no: u32,
    pub dmabuf_fd: i32,
    pub dmabuf_vaddr: usize,
    pub dmabuf_paddr: u64,
    pub tpu_path: TpuSubmitPath,
    pub ret: i32,
}

pub struct TpuKernelWork {
    pub task_list: VecDeque<TpuTaskNode>,
    pub done_list: VecDeque<TpuTaskNode>,
}

impl Default for TpuKernelWork {
    fn default() -> Self { Self { task_list: VecDeque::new(), done_list: VecDeque::new() } }
}

struct TpuDeviceInner {
    tdma: TdmaRegs,
    tiu: TiuRegs,
    state: TpuState,
    runtime: TpuRuntimeState,
    kernel_work: TpuKernelWork,
}

pub struct TpuDevice {
    inner: Mutex<TpuDeviceInner>,
    seq_counter: AtomicU32,
    ion_manager: Option<Arc<IonBufferManager>>,
}

impl TpuDevice {
    pub unsafe fn new() -> Self {
        let virt_offset = 0xffff_ffc0_0000_0000u64 as isize;
        let tdma_vaddr = (TDMA_PHYS_BASE as isize + virt_offset) as *mut u8;
        let tiu_vaddr = (TIU_PHYS_BASE as isize + virt_offset) as *mut u8;
        Self {
            inner: Mutex::new(TpuDeviceInner {
                tdma: unsafe { TdmaRegs::new(tdma_vaddr) },
                tiu: unsafe { TiuRegs::new(tiu_vaddr) },
                state: TpuState::Uninitialized,
                runtime: TpuRuntimeState::default(),
                kernel_work: TpuKernelWork::default(),
            }),
            seq_counter: AtomicU32::new(0),
            ion_manager: Some(global_ion_buffer_manager()),
        }
    }

    pub fn init(&self) -> Result<(), TpuError> {
        let mut inner = self.inner.lock();
        super::platform::resync_cmd_id(&inner.tdma, &inner.tiu);
        inner.state = TpuState::Idle;
        inner.runtime = TpuRuntimeState::default();
        info!("TPU device initialized");
        Ok(())
    }

    pub fn state(&self) -> TpuState { self.inner.lock().state }
    pub fn is_ready(&self) -> bool { self.inner.lock().state == TpuState::Idle }

    fn next_seq_no(&self) -> u32 { self.seq_counter.fetch_add(1, Ordering::SeqCst) }

    fn submit_dmabuf(&self, arg: usize) -> Result<usize, TpuError> {
        let submit_arg = unsafe { &*(arg as *const CviSubmitDmaArg) };
        let fd = submit_arg.fd as i32;
        let file = get_file_like(fd).map_err(|_| TpuError::InvalidDmabuf)?;
        let ion_file: Arc<IonBufferFile> = file.downcast_arc::<IonBufferFile>()
            .map_err(|_| TpuError::InvalidDmabuf)?;
        let buffer_info = ion_file.info();
        let ion_manager = self.ion_manager.as_ref().ok_or(TpuError::NotInitialized)?;
        let handle = IonHandle(buffer_info.handle);
        let buffer = ion_manager.get_buffer(handle).map_err(|_| TpuError::InvalidDmabuf)?;
        let dmabuf_vaddr = buffer.dma_info.cpu_addr.as_ptr() as usize;
        let dmabuf_paddr = buffer.dma_info.bus_addr.as_u64();
        let task = TpuTaskNode {
            pid: 0, seq_no: submit_arg.seq_no, dmabuf_fd: submit_arg.fd,
            dmabuf_vaddr, dmabuf_paddr, tpu_path: TpuSubmitPath::DesNormal, ret: 0,
        };
        let mut inner = self.inner.lock();
        inner.kernel_work.task_list.push_back(task);
        self.process_task_locked(&mut inner)?;
        Ok(0)
    }

    fn process_task_locked(&self, inner: &mut TpuDeviceInner) -> Result<(), TpuError> {
        while let Some(mut task) = inner.kernel_work.task_list.pop_front() {
            super::platform::resync_cmd_id(&inner.tdma, &inner.tiu);
            inner.runtime.irq_received = false;
            let result = self.run_dmabuf_internal(inner, task.dmabuf_vaddr as *const u8, task.dmabuf_paddr);
            task.ret = match result { Ok(_) => 0, Err(_) => -1 };
            inner.kernel_work.done_list.push_back(task);
        }
        Ok(())
    }

    fn run_dmabuf_internal(&self, inner: &mut TpuDeviceInner, dmabuf_vaddr: *const u8, dmabuf_paddr: u64) -> Result<(), TpuError> {
        if inner.state != TpuState::Idle && inner.state != TpuState::Uninitialized {
            return Err(TpuError::NotInitialized);
        }
        inner.state = TpuState::Running;
        let timeout_counter = Cell::new(0u64);
        let timeout_limit = 1_000_000_000u64;
        let wait_irq = || -> Result<(), TpuError> {
            let mut counter = timeout_counter.get();
            while counter < timeout_limit {
                counter += 1;
                timeout_counter.set(counter);
                core::hint::spin_loop();
                if counter > 10000 { break; }
            }
            if counter >= timeout_limit { return Err(TpuError::Timeout); }
            Ok(())
        };
        let timeout_checker = || -> bool { timeout_counter.get() > timeout_limit };
        let tdma = &inner.tdma as *const TdmaRegs;
        let tiu = &inner.tiu as *const TiuRegs;
        let runtime = &mut inner.runtime;
        let result = unsafe {
            super::platform::run_dmabuf(&*tdma, &*tiu, dmabuf_vaddr, dmabuf_paddr, runtime, wait_irq, timeout_checker)
        };
        inner.state = TpuState::Idle;
        result
    }

    fn wait_dmabuf(&self, arg: usize) -> Result<usize, TpuError> {
        let wait_arg = unsafe { &mut *(arg as *mut CviWaitDmaArg) };
        let mut inner = self.inner.lock();
        let mut found_idx = None;
        for (idx, task) in inner.kernel_work.done_list.iter().enumerate() {
            if task.seq_no == wait_arg.seq_no { found_idx = Some(idx); break; }
        }
        if let Some(idx) = found_idx {
            let task = inner.kernel_work.done_list.remove(idx).unwrap();
            wait_arg.ret = task.ret;
            Ok(0)
        } else {
            wait_arg.ret = -1;
            Err(TpuError::NotInitialized)
        }
    }

    fn cache_flush(&self, _arg: usize) -> Result<usize, TpuError> {
        #[cfg(target_arch = "riscv64")]
        unsafe { core::arch::asm!("fence iorw, iorw"); }
        Ok(0)
    }

    fn cache_invalidate(&self, _arg: usize) -> Result<usize, TpuError> {
        #[cfg(target_arch = "riscv64")]
        unsafe { core::arch::asm!("fence iorw, iorw"); }
        Ok(0)
    }

    fn dmabuf_flush_fd(&self, arg: usize) -> Result<usize, TpuError> {
        if let Some(ref ion_manager) = self.ion_manager {
            let handle = IonHandle(arg as u32);
            if let Ok(_buffer) = ion_manager.get_buffer(handle) {
                #[cfg(target_arch = "riscv64")]
                unsafe { core::arch::asm!("fence iorw, iorw"); }
            }
        }
        Ok(0)
    }

    fn dmabuf_invld_fd(&self, arg: usize) -> Result<usize, TpuError> {
        if let Some(ref ion_manager) = self.ion_manager {
            let handle = IonHandle(arg as u32);
            if let Ok(_buffer) = ion_manager.get_buffer(handle) {
                #[cfg(target_arch = "riscv64")]
                unsafe { core::arch::asm!("fence iorw, iorw"); }
            }
        }
        Ok(0)
    }

    pub fn suspend(&self) -> Result<(), TpuError> {
        let mut inner = self.inner.lock();
        if inner.state == TpuState::Suspended { return Ok(()); }
        let tdma = &inner.tdma as *const TdmaRegs;
        let tiu = &inner.tiu as *const TiuRegs;
        let reg_backup = &mut inner.runtime.reg_backup;
        unsafe { super::platform::backup_registers(&*tdma, &*tiu, reg_backup); }
        inner.state = TpuState::Suspended;
        Ok(())
    }

    pub fn resume(&self) -> Result<(), TpuError> {
        let mut inner = self.inner.lock();
        if inner.state != TpuState::Suspended { return Err(TpuError::NotInitialized); }
        let tdma = &inner.tdma as *const TdmaRegs;
        let tiu = &inner.tiu as *const TiuRegs;
        let reg_backup = &inner.runtime.reg_backup;
        unsafe { super::platform::restore_registers(&*tdma, &*tiu, reg_backup); }
        inner.state = TpuState::Idle;
        Ok(())
    }

    pub fn reset(&self) {
        let mut inner = self.inner.lock();
        super::platform::resync_cmd_id(&inner.tdma, &inner.tiu);
        inner.runtime = TpuRuntimeState::default();
        inner.state = TpuState::Idle;
    }
}

unsafe impl Send for TpuDevice {}
unsafe impl Sync for TpuDevice {}

impl DeviceOps for TpuDevice {
    fn read_at(&self, _buf: &mut [u8], _offset: u64) -> axfs_ng_vfs::VfsResult<usize> { Ok(0) }
    fn write_at(&self, _buf: &[u8], _offset: u64) -> axfs_ng_vfs::VfsResult<usize> { Ok(0) }

    fn ioctl(&self, cmd: u32, arg: usize) -> axfs_ng_vfs::VfsResult<usize> {
        warn!("TPU ioctl: cmd={:#x} arg={:#x}", cmd, arg);
        let result = match cmd {
            CVITPU_SUBMIT_DMABUF  => self.submit_dmabuf(arg),
            CVITPU_DMABUF_FLUSH_FD => self.dmabuf_flush_fd(arg),
            CVITPU_DMABUF_INVLD_FD => self.dmabuf_invld_fd(arg),
            CVITPU_DMABUF_FLUSH   => self.cache_flush(arg),
            CVITPU_DMABUF_INVLD   => self.cache_invalidate(arg),
            CVITPU_WAIT_DMABUF    => self.wait_dmabuf(arg),
            CVITPU_PIO_MODE | CVITPU_LOAD_TEE | CVITPU_SUBMIT_TEE | CVITPU_UNLOAD_TEE => Ok(0),
            _ => {
                warn!("TPU unknown ioctl cmd: {:#x}", cmd);
                Err(TpuError::NotInitialized)
            }
        };
        match result {
            Ok(v) => Ok(v),
            Err(_) => Err(ax_errno::AxError::Unsupported),
        }
    }

    fn as_any(&self) -> &dyn core::any::Any { self }
}
