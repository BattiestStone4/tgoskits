use alloc::borrow::Cow;

use ax_errno::AxResult;
use ax_memory_addr::PhysAddrRange;
use axpoll::{IoEvents, Pollable};

use crate::pseudofs::DeviceOps;

use super::{FileLike, IoDst, IoSrc, Kstat};

#[derive(Debug, Clone)]
pub struct IonBufferInfo {
    pub phys_addr: usize,
    pub size: usize,
    pub handle: u32,
}

pub struct IonBufferFile {
    info: IonBufferInfo,
}

impl IonBufferFile {
    pub fn new(info: IonBufferInfo) -> Self {
        Self { info }
    }

    pub fn phys_range(&self) -> PhysAddrRange {
        PhysAddrRange::from_start_size(
            ax_memory_addr::PhysAddr::from(self.info.phys_addr),
            self.info.size,
        )
    }

    pub fn info(&self) -> &IonBufferInfo {
        &self.info
    }
}

impl Pollable for IonBufferFile {
    fn poll(&self) -> IoEvents {
        IoEvents::IN | IoEvents::OUT
    }

    fn register(&self, _context: &mut core::task::Context<'_>, _events: IoEvents) {}
}

impl FileLike for IonBufferFile {
    fn read(&self, _dst: &mut IoDst) -> AxResult<usize> {
        Err(ax_errno::AxError::InvalidInput)
    }

    fn write(&self, _src: &mut IoSrc) -> AxResult<usize> {
        Err(ax_errno::AxError::InvalidInput)
    }

    fn stat(&self) -> AxResult<Kstat> {
        Ok(Kstat {
            size: self.info.size as u64,
            ..Default::default()
        })
    }

    fn path(&self) -> Cow<'_, str> {
        Cow::Borrowed("/dev/ion_buffer")
    }

    fn device_mmap(&self, _offset: u64) -> AxResult<crate::pseudofs::DeviceMmap> {
        Ok(crate::pseudofs::DeviceMmap::Physical(self.phys_range()))
    }

    fn ioctl(&self, cmd: u32, _arg: usize) -> AxResult<usize> {
        warn!("IonBufferFile ioctl: cmd=0x{:x}", cmd);
        Err(ax_errno::AxError::Unsupported)
    }
}

impl Drop for IonBufferFile {
    fn drop(&mut self) {
        use crate::pseudofs::dev::ION_DEVICE;
        use crate::pseudofs::dev::ion::types::ioctl::{ION_IOC_FREE, IonHandleData};
        if let Some(dev) = ION_DEVICE.get() {
            let handle_data = IonHandleData { handle: self.info.handle };
            let _ = dev.ioctl(ION_IOC_FREE, &handle_data as *const _ as usize);
        }
    }
}
