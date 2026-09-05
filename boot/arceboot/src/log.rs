//! Logger implementation for ArceBoot, publishing directly to the platform
//! console. The runtime has no scheduling or IRQ infrastructure, so the
//! records are formatted and written inline.

struct LogIfImpl;

/// Formats `fmt::Arguments` into the platform console without allocation.
struct ConsoleWriter;

impl core::fmt::Write for ConsoleWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        ax_hal::console::write_bytes(s.as_bytes());
        Ok(())
    }
}

#[ax_crate_interface::impl_interface]
impl ax_log::LogIf for LogIfImpl {
    fn try_publish(
        _meta: ax_log::RecordMeta,
        args: core::fmt::Arguments<'_>,
    ) -> ax_log::PublishStatus {
        let mut writer = ConsoleWriter;
        if core::fmt::write(&mut writer, args).is_ok() {
            ax_log::PublishStatus::Published
        } else {
            ax_log::PublishStatus::Dropped
        }
    }

    fn emergency_write(args: core::fmt::Arguments<'_>) -> usize {
        let mut writer = ConsoleWriter;
        core::fmt::write(&mut writer, args).map_or(0, |_| 1)
    }
}
