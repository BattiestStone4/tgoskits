use alloc::sync::Arc;
use core::{any::Any, ptr};

use ax_errno::AxError;
use ax_memory_addr::PhysAddrRange;
use axfs_ng_vfs::{NodeFlags, VfsResult};

use crate::pseudofs::{DeviceMmap, DeviceOps};

use super::{
    buffer::IonBufferManager,
    global_ion_buffer_manager,
    heap::IonHeapManager,
    types::{ioctl::*, *},
};
use crate::file::{add_file_like, ion::IonBufferFile};

pub struct IonDevice {
    heap_manager: IonHeapManager,
    buffer_manager: Arc<IonBufferManager>,
}

impl IonDevice {
    pub fn new() -> Self {
        Self {
            heap_manager: IonHeapManager::new(),
            buffer_manager: global_ion_buffer_manager(),
        }
    }

    fn handle_alloc(&self, user_ptr: usize) -> VfsResult<usize> {
        let alloc_data = unsafe { ptr::read(user_ptr as *const IonAllocData) };

        let heap_type = if (alloc_data.heap_id_mask & (1 << IonHeapType::DmaCoherent as u32)) != 0 {
            IonHeapType::DmaCoherent
        } else if (alloc_data.heap_id_mask & (1 << IonHeapType::Carveout as u32)) != 0 {
            IonHeapType::Carveout
        } else if (alloc_data.heap_id_mask & (1 << IonHeapType::System as u32)) != 0 {
            IonHeapType::System
        } else {
            return Err(AxError::InvalidInput);
        };
        warn!("ION_ALLOC req: len={:#x} heap_mask={:#x} flags={:#x}",
              alloc_data.len, alloc_data.heap_id_mask, alloc_data.flags);

        let buffer = self
            .heap_manager
            .alloc_buffer(alloc_data.len as usize, 1, heap_type, IonFlags(alloc_data.flags))
            .map_err(AxError::from)?;

        self.buffer_manager
            .register_buffer(buffer.clone())
            .map_err(AxError::from)?;

        let phys_addr = buffer.dma_info.bus_addr.as_u64() as usize;

        // Pre-fill with a known sentinel pattern, so userspace can verify
        // the mmap virtual address really maps to this allocation.
        unsafe {
            let p = buffer.dma_info.cpu_addr.as_ptr() as *mut u32;
            *p = 0xCAFEBABE;
            *p.add(1) = 0xDEADBEEF;
            *p.add(2) = phys_addr as u32;
            *p.add(3) = (phys_addr >> 32) as u32;
        }
        warn!("ION_ALLOC done: paddr={:#x} cpu_addr={:p} size={:#x} sentinel=CAFEBABE/DEADBEEF",
              phys_addr, buffer.dma_info.cpu_addr.as_ptr(), buffer.size);
        let ion_file = IonBufferFile::new(crate::file::ion::IonBufferInfo {
            phys_addr,
            size: buffer.size,
            handle: buffer.handle.as_u32(),
        });
        let fd = add_file_like(Arc::new(ion_file), false)
            .map_err(|_| AxError::TooManyOpenFiles)?;

        let mut result_data = alloc_data;
        result_data.fd = fd as u32;
        result_data.paddr = phys_addr as u64;
        unsafe { ptr::write(user_ptr as *mut IonAllocData, result_data) };

        Ok(0)
    }

    fn handle_free(&self, user_ptr: usize) -> VfsResult<usize> {
        let handle_data = unsafe { ptr::read(user_ptr as *const IonHandleData) };
        let handle = IonHandle(handle_data.handle);
        let buffer = self.buffer_manager.unregister_buffer(handle).map_err(AxError::from)?;
        self.heap_manager.free_buffer(buffer).map_err(AxError::from)?;
        Ok(0)
    }

    fn handle_import(&self, user_ptr: usize) -> VfsResult<usize> {
        let fd_data = unsafe { ptr::read(user_ptr as *const IonFdData) };
        let handle = IonHandle(fd_data.fd as u32);
        let mut result_data = fd_data;
        result_data.handle = handle.as_u32();
        unsafe { ptr::write(user_ptr as *mut IonFdData, result_data) };
        Ok(0)
    }

    fn handle_heap_query(&self, user_ptr: usize) -> VfsResult<usize> {
        let mut heap_query = unsafe { ptr::read(user_ptr as *const IonHeapQuery) };

        let supported_heaps = [
            (IonHeapType::System, "system", 0u32),
            (IonHeapType::DmaCoherent, "dma_coherent", 1u32),
            (IonHeapType::Carveout, "carveout", 2u32),
        ];
        let available = supported_heaps.len() as u32;
        let requested = heap_query.cnt.min(available);

        if heap_query.heaps != 0 && requested > 0 {
            let heap_data_ptr = heap_query.heaps as *mut IonHeapData;
            for (i, &(heap_type, name, heap_id)) in supported_heaps.iter().enumerate().take(requested as usize) {
                let mut heap_data = IonHeapData {
                    name: [0; MAX_HEAP_NAME],
                    type_: heap_type as u32,
                    heap_id,
                    reserved0: 0,
                    reserved1: 0,
                    reserved2: 0,
                };
                let name_bytes = name.as_bytes();
                let copy_len = name_bytes.len().min(MAX_HEAP_NAME - 1);
                heap_data.name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
                unsafe { ptr::write(heap_data_ptr.add(i), heap_data) };
            }
        }

        heap_query.cnt = available;
        unsafe { ptr::write(user_ptr as *mut IonHeapQuery, heap_query) };
        Ok(0)
    }
}

impl DeviceOps for IonDevice {
    fn read_at(&self, _buf: &mut [u8], _offset: u64) -> VfsResult<usize> { Ok(0) }
    fn write_at(&self, _buf: &[u8], _offset: u64) -> VfsResult<usize> { Ok(0) }

    fn ioctl(&self, cmd: u32, arg: usize) -> VfsResult<usize> {
        match cmd {
            ION_IOC_HEAP_QUERY => self.handle_heap_query(arg),
            ION_IOC_ALLOC      => self.handle_alloc(arg),
            ION_IOC_FREE       => self.handle_free(arg),
            ION_IOC_IMPORT     => self.handle_import(arg),
            _ => {
                warn!("Unsupported Ion ioctl command: 0x{:x}", cmd);
                Err(AxError::Unsupported)
            }
        }
    }

    fn as_any(&self) -> &dyn Any { self }

    fn flags(&self) -> NodeFlags { NodeFlags::NON_CACHEABLE }

    fn mmap(&self, offset: u64) -> DeviceMmap {
        let handle = IonHandle(offset as u32);
        match self.buffer_manager.get_buffer(handle) {
            Ok(buffer) => {
                let phys_addr = buffer.dma_info.bus_addr.as_u64() as usize;
                buffer.set_mapped();
                DeviceMmap::Physical(PhysAddrRange::from_start_size(
                    ax_memory_addr::PhysAddr::from(phys_addr),
                    buffer.size,
                ))
            }
            Err(_) => DeviceMmap::None,
        }
    }
}

impl Drop for IonDevice {
    fn drop(&mut self) {
        self.buffer_manager.cleanup_all();
    }
}
