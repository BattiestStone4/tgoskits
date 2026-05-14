mod buffer;
mod device;
mod error;
mod heap;
pub mod types;

use alloc::sync::Arc;
use spin::Once;

pub use buffer::IonBufferManager;
pub use device::IonDevice;
pub use types::IonHandle;

static GLOBAL_ION_BUFFER_MANAGER: Once<Arc<IonBufferManager>> = Once::new();

pub fn global_ion_buffer_manager() -> Arc<IonBufferManager> {
    GLOBAL_ION_BUFFER_MANAGER
        .call_once(|| Arc::new(IonBufferManager::new()))
        .clone()
}
