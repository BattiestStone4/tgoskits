use alloc::sync::Arc;
use core::alloc::Layout;

use ax_dma::{self, DMAInfo};

use super::{
    error::{IonError, IonResult},
    types::{IonBuffer, IonFlags, IonHeapType},
};

pub struct IonHeapManager;

impl IonHeapManager {
    pub const fn new() -> Self {
        Self
    }

    pub fn alloc_buffer(
        &self,
        size: usize,
        align: usize,
        heap_type: IonHeapType,
        flags: IonFlags,
    ) -> IonResult<Arc<IonBuffer>> {
        if size == 0 {
            return Err(IonError::InvalidArg);
        }
        let dma_info = match heap_type {
            IonHeapType::System | IonHeapType::DmaCoherent => self.alloc_dma_buffer(size, align)?,
            IonHeapType::Carveout => {
                warn!("Carveout heap not implemented, using DMA heap instead");
                self.alloc_dma_buffer(size, align)?
            }
        };
        Ok(Arc::new(IonBuffer::new(dma_info, size, heap_type, flags)))
    }

    pub fn free_buffer(&self, buffer: Arc<IonBuffer>) -> IonResult<()> {
        let layout = Layout::from_size_align(buffer.size, 1).map_err(|_| IonError::InvalidArg)?;
        unsafe { ax_dma::dealloc_coherent(buffer.dma_info, layout) };
        Ok(())
    }

    fn alloc_dma_buffer(&self, size: usize, align: usize) -> IonResult<DMAInfo> {
        let align = align.max(1);
        let layout = Layout::from_size_align(size, align).map_err(|_| IonError::InvalidArg)?;
        unsafe { ax_dma::alloc_coherent(layout).map_err(|_| IonError::NoMemory) }
    }
}
