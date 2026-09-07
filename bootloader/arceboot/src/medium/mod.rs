#[cfg(feature = "ramdisk_cpio")]
pub mod ramdisk_cpio;
// The virtio-blk/fs path is not ported to tgoskits yet (needs a
// virtio-blk rdif driver + ax-fs-ng adapter); see the migration notes.
#[cfg(feature = "fs")]
pub mod virtio_disk;
