use alloc::{collections::BTreeMap, sync::Arc};

use ax_sync::Mutex;

use super::{
    error::{IonError, IonResult},
    types::{IonBuffer, IonHandle},
};

pub struct IonBufferManager {
    buffers: Mutex<BTreeMap<IonHandle, Arc<IonBuffer>>>,
}

impl IonBufferManager {
    pub const fn new() -> Self {
        Self {
            buffers: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn register_buffer(&self, buffer: Arc<IonBuffer>) -> IonResult<()> {
        let mut buffers = self.buffers.lock();
        let handle = buffer.handle;
        if buffers.contains_key(&handle) {
            return Err(IonError::BufferExists);
        }
        buffers.insert(handle, buffer);
        Ok(())
    }

    pub fn unregister_buffer(&self, handle: IonHandle) -> IonResult<Arc<IonBuffer>> {
        let mut buffers = self.buffers.lock();
        buffers.remove(&handle).ok_or(IonError::BufferNotFound)
    }

    pub fn get_buffer(&self, handle: IonHandle) -> IonResult<Arc<IonBuffer>> {
        let buffers = self.buffers.lock();
        buffers.get(&handle).cloned().ok_or(IonError::BufferNotFound)
    }

    pub fn cleanup_all(&self) {
        let mut buffers = self.buffers.lock();
        let count = buffers.len();
        buffers.clear();
        if count > 0 {
            warn!("Cleaned up {} Ion buffers", count);
        }
    }
}
