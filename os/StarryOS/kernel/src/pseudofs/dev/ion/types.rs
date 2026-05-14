use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use ax_dma::DMAInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum IonHeapType {
    System      = 0,
    DmaCoherent = 1,
    Carveout    = 2,
}

impl TryFrom<u32> for IonHeapType {
    type Error = ();
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::System),
            1 => Ok(Self::DmaCoherent),
            2 => Ok(Self::Carveout),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct IonFlags(pub u32);

impl IonFlags {
    pub const CACHED: Self = Self(1 << 0);
    pub const CACHED_NEEDS_SYNC: Self = Self(1 << 1);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct IonHandle(pub u32);

impl IonHandle {
    pub fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug)]
pub struct IonBuffer {
    pub handle: IonHandle,
    pub dma_info: DMAInfo,
    pub size: usize,
    pub heap_type: IonHeapType,
    pub flags: IonFlags,
    pub ref_count: AtomicUsize,
    pub mapped: AtomicUsize,
}

impl IonBuffer {
    pub fn new(dma_info: DMAInfo, size: usize, heap_type: IonHeapType, flags: IonFlags) -> Self {
        Self {
            handle: IonHandle::new(),
            dma_info,
            size,
            heap_type,
            flags,
            ref_count: AtomicUsize::new(1),
            mapped: AtomicUsize::new(0),
        }
    }

    pub fn inc_ref(&self) -> usize {
        self.ref_count.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn dec_ref(&self) -> usize {
        let old = self.ref_count.fetch_sub(1, Ordering::SeqCst);
        if old > 0 { old - 1 } else { 0 }
    }

    pub fn ref_count(&self) -> usize {
        self.ref_count.load(Ordering::SeqCst)
    }

    pub fn set_mapped(&self) {
        self.mapped.store(1, Ordering::SeqCst);
    }

    pub fn is_mapped(&self) -> bool {
        self.mapped.load(Ordering::SeqCst) != 0
    }
}

unsafe impl Send for IonBuffer {}
unsafe impl Sync for IonBuffer {}

pub const MAX_HEAP_NAME: usize = 32;
pub const MAX_ION_BUFFER_NAME: usize = 32;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IonAllocData {
    pub len: u64,
    pub heap_id_mask: u32,
    pub flags: u32,
    pub fd: u32,
    pub unused: u32,
    pub paddr: u64,
    pub name: [u8; MAX_ION_BUFFER_NAME],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IonFdData {
    pub fd: i32,
    pub handle: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IonHandleData {
    pub handle: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct IonHeapData {
    pub name: [u8; MAX_HEAP_NAME],
    pub type_: u32,
    pub heap_id: u32,
    pub reserved0: u32,
    pub reserved1: u32,
    pub reserved2: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IonHeapQuery {
    pub cnt: u32,
    pub reserved0: u32,
    pub heaps: u64,
    pub reserved1: u32,
    pub reserved2: u32,
}

pub mod ioctl {
    pub use super::*;

    pub const ION_IOC_MAGIC: u8 = b'I';

    pub const ION_IOC_ALLOC: u32 = ioctl_iowr!(ION_IOC_MAGIC, 0, IonAllocData);
    pub const ION_IOC_HEAP_QUERY: u32 = ioctl_iowr!(ION_IOC_MAGIC, 8, IonHeapQuery);
    pub const ION_IOC_FREE: u32 = ioctl_iow!(ION_IOC_MAGIC, 1, IonHandleData);
    pub const ION_IOC_IMPORT: u32 = ioctl_iowr!(ION_IOC_MAGIC, 5, IonFdData);
}

macro_rules! ioctl_iowr {
    ($magic:expr, $nr:expr, $ty:ty) => {
        (3u32 << 30)
            | (($magic as u32) << 8)
            | ($nr as u32)
            | ((core::mem::size_of::<$ty>() as u32) << 16)
    };
}

macro_rules! ioctl_iow {
    ($magic:expr, $nr:expr, $ty:ty) => {
        (1u32 << 30)
            | (($magic as u32) << 8)
            | ($nr as u32)
            | ((core::mem::size_of::<$ty>() as u32) << 16)
    };
}

pub(crate) use ioctl_iow;
pub(crate) use ioctl_iowr;
