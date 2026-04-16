// riscv/virtio_blk.rs -- Virtio MMIO block device (Phase 36)
//
// Minimal virtio block device emulation using the MMIO transport.
// Provides a simple disk image backed by a Vec<u8>.
//
// MMIO address map (base at 0x1000_1000, matching QEMU virt):
//   0x00  MagicValue       (R)  = 0x74726976 ("virt")
//   0x04  Version          (R)  = 2 (virtio 1.0)
//   0x08  DeviceID         (R)  = 2 (block device)
//   0x0C  VendorID         (R)  = 0x5555_5555
//   0x10  DeviceFeatures   (R)  feature bits
//   0x14  DeviceFeaturesSel(W)  feature selection
//   0x20  DriverFeatures   (W)  acknowledged features
//   0x28  Status           (R/W) device status
//   0x30  QueueSel         (W)  queue selector
//   0x34  QueueNumMax      (R)  max queue size
//   0x38  QueueNum         (W)  queue size
//   0x44  QueueReady       (R)  queue ready
//   0x50  QueueNotify      (W)  kick queue
//   0x60  InterruptStatus  (R)  interrupt status
//   0x70  ConfigGeneration (R)  config generation
//   0x100 Config           (R)  device config (capacity, etc.)
//
// For simplicity, we implement a minimal virt queue with direct
// descriptor processing (no real ring buffer, just direct R/W ops).

/// Virtio MMIO base address.
pub const VIRTIO_BASE: u64 = 0x1000_1000;
/// Virtio MMIO size.
pub const VIRTIO_SIZE: u64 = 0x1000;

/// Register offsets.
const MAGIC_VALUE: u64 = 0x00;
const VERSION: u64 = 0x04;
const DEVICE_ID: u64 = 0x08;
const VENDOR_ID: u64 = 0x0C;
const DEVICE_FEATURES: u64 = 0x10;
const DEVICE_FEATURES_SEL: u64 = 0x14;
const DRIVER_FEATURES: u64 = 0x20;
const STATUS: u64 = 0x28;
const QUEUE_SEL: u64 = 0x30;
const QUEUE_NUM_MAX: u64 = 0x34;
const QUEUE_NUM: u64 = 0x38;
const QUEUE_READY: u64 = 0x44;
const QUEUE_NOTIFY: u64 = 0x50;
const INTERRUPT_STATUS: u64 = 0x60;
const CONFIG_GENERATION: u64 = 0x70;
const CONFIG: u64 = 0x100;

/// Virtio magic value ("virt" in little-endian).
const VIRTIO_MAGIC: u32 = 0x7472_6976;

/// Virtio block device ID.
const VIRTIO_BLK_ID: u32 = 2;

/// Block size (512 bytes).
pub const SECTOR_SIZE: u64 = 512;

/// Maximum virt queue size.
const QUEUE_SIZE: u32 = 16;

/// Virtio block device capacity (in 512-byte sectors).
const DEFAULT_SECTORS: u64 = 2048; // 1MB disk

/// Config capacity high word offset.
const CONFIG_CAPACITY_HI: u64 = CONFIG + 4;


/// Virtio block request header (16 bytes).
///   type: u32 (0=read, 1=write)
///   reserved: u32
///   sector: u64
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
struct BlkRequest {
    req_type: u32,
    sector: u64,
}

#[allow(dead_code)]
const VIRTIO_STATUS_ACKNOWLEDGE: u32 = 1;
#[allow(dead_code)]
const VIRTIO_STATUS_DRIVER: u32 = 2;
#[allow(dead_code)]
const VIRTIO_STATUS_FEATURES_OK: u32 = 8;
#[allow(dead_code)]
const VIRTIO_STATUS_DRIVER_OK: u32 = 4;
#[allow(dead_code)]
const VIRTIO_STATUS_FAILED: u32 = 128;

/// Virtio block device.
#[derive(Clone, Debug)]
pub struct VirtioBlk {
    /// Disk image.
    pub disk: Vec<u8>,
    /// Device status register.
    pub status: u32,
    /// Current queue selector.
    pub queue_sel: u32,
    /// Queue sizes.
    pub queue_num: [u32; 2],
    /// Queue ready flags.
    pub queue_ready: [bool; 2],
    /// Driver features.
    pub driver_features: u32,
    /// Device features (what we support).
    pub device_features: u32,
    /// Interrupt status.
    pub int_status: u32,
    /// Device features selector.
    pub dev_features_sel: u32,
}

impl Default for VirtioBlk {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtioBlk {
    pub fn new() -> Self {
        Self {
            disk: vec![0u8; (DEFAULT_SECTORS * SECTOR_SIZE) as usize],
            status: 0,
            queue_sel: 0,
            queue_num: [QUEUE_SIZE; 2],
            queue_ready: [false; 2],
            driver_features: 0,
            device_features: 0, // Minimal features
            int_status: 0,
            dev_features_sel: 0,
        }
    }

    /// Create a virtio block device with a custom disk image.
    pub fn with_disk(disk: Vec<u8>) -> Self {
        let mut dev = Self::new();
        dev.disk = disk;
        dev
    }

    /// Check if address is in virtio MMIO range.
    pub fn contains(addr: u64) -> bool {
        (VIRTIO_BASE..VIRTIO_BASE + VIRTIO_SIZE).contains(&addr)
    }

    /// Read a 32-bit word from virtio MMIO space.
    pub fn read(&self, addr: u64) -> Option<u32> {
        let offset = addr - VIRTIO_BASE;
        match offset {
            MAGIC_VALUE => Some(VIRTIO_MAGIC),
            VERSION => Some(2), // virtio 1.0
            DEVICE_ID => Some(VIRTIO_BLK_ID),
            VENDOR_ID => Some(0x5555_5555),
            DEVICE_FEATURES => {
                if self.dev_features_sel == 0 {
                    Some(self.device_features)
                } else {
                    Some(0)
                }
            }
            STATUS => Some(self.status),
            QUEUE_NUM_MAX => Some(QUEUE_SIZE),
            QUEUE_READY => {
                let idx = self.queue_sel as usize;
                if idx < 2 {
                    Some(if self.queue_ready[idx] { 1 } else { 0 })
                } else {
                    Some(0)
                }
            }
            INTERRUPT_STATUS => Some(self.int_status),
            CONFIG_GENERATION => Some(0),
            CONFIG => {
                // First 8 bytes of config: capacity in sectors (little-endian u64)
                let cap = (self.disk.len() as u64) / SECTOR_SIZE;
                Some((cap & 0xFFFF_FFFF) as u32)
            }
            CONFIG_CAPACITY_HI => {
                let cap = (self.disk.len() as u64) / SECTOR_SIZE;
                Some((cap >> 32) as u32)
            }
            _ => Some(0),
        }
    }

    /// Write a 32-bit word to virtio MMIO space.
    pub fn write(&mut self, addr: u64, val: u32) -> bool {
        let offset = addr - VIRTIO_BASE;
        match offset {
            DEVICE_FEATURES_SEL => {
                self.dev_features_sel = val;
            }
            DRIVER_FEATURES => {
                self.driver_features = val;
            }
            STATUS => {
                self.status = val;
            }
            QUEUE_SEL => {
                self.queue_sel = val;
            }
            QUEUE_NUM => {
                let idx = self.queue_sel as usize;
                if idx < 2 {
                    self.queue_num[idx] = val;
                }
            }
            QUEUE_READY => {
                let idx = self.queue_sel as usize;
                if idx < 2 {
                    self.queue_ready[idx] = val != 0;
                }
            }
            QUEUE_NOTIFY => {
                // Guest kicks the queue -- in a real implementation we'd
                // process the virtqueue. For now, this is a no-op.
                // The disk operations are done directly through read/write helpers.
            }
            _ => {}
        }
        true
    }

    /// Read sectors from the disk image.
    /// Returns the data at the given sector offset, or None if out of range.
    pub fn read_sectors(&self, sector: u64, count: u32) -> Option<Vec<u8>> {
        let start = (sector * SECTOR_SIZE) as usize;
        let len = (count as usize) * (SECTOR_SIZE as usize);
        if start + len > self.disk.len() {
            return None;
        }
        Some(self.disk[start..start + len].to_vec())
    }

    /// Write sectors to the disk image.
    /// Returns true if the write succeeded.
    pub fn write_sectors(&mut self, sector: u64, data: &[u8]) -> bool {
        let start = (sector * SECTOR_SIZE) as usize;
        if start + data.len() > self.disk.len() {
            return false;
        }
        self.disk[start..start + data.len()].copy_from_slice(data);
        true
    }

    /// Disk capacity in 512-byte sectors.
    pub fn capacity(&self) -> u64 {
        (self.disk.len() as u64) / SECTOR_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_virtio_defaults() {
        let dev = VirtioBlk::new();
        assert_eq!(dev.status, 0);
        assert_eq!(dev.disk.len(), (DEFAULT_SECTORS * SECTOR_SIZE) as usize);
    }

    #[test]
    fn read_magic() {
        let dev = VirtioBlk::new();
        assert_eq!(dev.read(VIRTIO_BASE), Some(VIRTIO_MAGIC));
    }

    #[test]
    fn read_version() {
        let dev = VirtioBlk::new();
        assert_eq!(dev.read(VIRTIO_BASE + VERSION), Some(2));
    }

    #[test]
    fn read_device_id() {
        let dev = VirtioBlk::new();
        assert_eq!(dev.read(VIRTIO_BASE + DEVICE_ID), Some(VIRTIO_BLK_ID));
    }

    #[test]
    fn write_read_status() {
        let mut dev = VirtioBlk::new();
        dev.write(VIRTIO_BASE + STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER);
        assert_eq!(
            dev.read(VIRTIO_BASE + STATUS),
            Some(VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER)
        );
    }

    #[test]
    fn config_capacity() {
        let dev = VirtioBlk::new();
        let lo = dev.read(VIRTIO_BASE + CONFIG).expect("read should succeed");
        let hi = dev.read(VIRTIO_BASE + CONFIG + 4).expect("read should succeed");
        let capacity = lo as u64 | ((hi as u64) << 32);
        assert_eq!(capacity, DEFAULT_SECTORS);
    }

    #[test]
    fn queue_num_max() {
        let dev = VirtioBlk::new();
        assert_eq!(dev.read(VIRTIO_BASE + QUEUE_NUM_MAX), Some(QUEUE_SIZE));
    }

    #[test]
    fn write_read_queue_ready() {
        let mut dev = VirtioBlk::new();
        dev.write(VIRTIO_BASE + QUEUE_SEL, 0);
        dev.write(VIRTIO_BASE + QUEUE_READY, 1);
        assert_eq!(dev.read(VIRTIO_BASE + QUEUE_READY), Some(1));
    }

    #[test]
    fn read_write_sectors() {
        let mut dev = VirtioBlk::new();
        let data = vec![0xABu8; 512];
        assert!(dev.write_sectors(0, &data));
        let read = dev.read_sectors(0, 1).expect("operation should succeed");
        assert_eq!(read, data);
    }

    #[test]
    fn read_out_of_range_returns_none() {
        let dev = VirtioBlk::new();
        assert!(dev.read_sectors(DEFAULT_SECTORS + 1, 1).is_none());
    }

    #[test]
    fn write_out_of_range_returns_false() {
        let mut dev = VirtioBlk::new();
        let data = vec![0u8; 512];
        assert!(!dev.write_sectors(DEFAULT_SECTORS + 1, &data));
    }

    #[test]
    fn custom_disk_image() {
        let dev = VirtioBlk::with_disk(vec![0xFFu8; 1024]);
        assert_eq!(dev.capacity(), 2); // 1024 / 512 = 2 sectors
    }
}
