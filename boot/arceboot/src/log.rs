//! Logger implementation for ArceBoot, publishing directly to the platform
//! console. The runtime has no scheduling or IRQ infrastructure.

struct LogIfImpl;

#[ax_crate_interface::impl_interface]
impl ax_log::LogIf for LogIfImpl {
    fn console_write_str(s: &str) {
        // ArceBoot prints through the SBI console rather than the platform
        // UART: some SBI firmware (e.g. RustSBI Prototyper) denies S-mode
        // access to the UART MMIO region in its PMP configuration, while the
        // legacy console call is always available.
        #[allow(deprecated)]
        for &byte in s.as_bytes() {
            sbi_rt::legacy::console_putchar(byte as usize);
        }
    }

    fn current_time() -> core::time::Duration {
        ax_hal::time::monotonic_time()
    }

    fn current_cpu_id() -> Option<usize> {
        Some(0)
    }

    fn current_task_id() -> Option<u64> {
        None
    }
}
