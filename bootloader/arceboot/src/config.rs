//! Build-time boot configuration.
//!
//! The upstream ArceBoot reads these values from the `axconfig` crate, which
//! is fed by an `axconfig-gen`-generated `.axconfig.toml`. tgoskits has no
//! `axconfig` crate, so this module reads the same values from environment
//! variables set by the build script (`scripts/build-arceboot.sh`), with
//! defaults matching `configs/platforms/riscv64-qemu-virt.toml`.

/// Boot-related configuration knobs.
pub mod boot {
    /// Whether the next-stage boot file is stored in a ramdisk.
    pub fn use_ramdisk() -> bool {
        matches!(option_env!("AX_USE_RAMDISK"), Some("1"))
    }

    /// Whether ArceBoot needs to load the ramdisk from a storage medium.
    ///
    /// Not supported yet in the tgoskits port (requires the `fs`/virtio-blk
    /// path), so this must stay `false`.
    pub fn load_ramdisk() -> bool {
        matches!(option_env!("AX_LOAD_RAMDISK"), Some("1"))
    }

    /// Ramdisk file path, used only when [`load_ramdisk`] is set.
    pub fn ramdisk_file() -> &'static str {
        option_env!("AX_RAMDISK_FILE").unwrap_or("/ramdisk.cpio")
    }

    /// Physical start address of a pre-loaded ramdisk (used when
    /// `use_ramdisk && !load_ramdisk`).
    pub fn ramdisk_start() -> usize {
        option_env!("AX_RAMDISK_START")
            .and_then(parse_hex)
            .unwrap_or(0x8400_0000)
    }

    /// Size of the pre-loaded ramdisk in bytes.
    pub fn ramdisk_size() -> usize {
        option_env!("AX_RAMDISK_SIZE")
            .and_then(parse_hex)
            .unwrap_or(0x4000_0000)
    }

    fn parse_hex(s: &str) -> Option<usize> {
        usize::from_str_radix(s.trim_start_matches("0x"), 16).ok()
    }
}
