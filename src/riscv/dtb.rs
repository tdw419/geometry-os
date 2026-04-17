// riscv/dtb.rs -- Device Tree Blob generator (Phase 36)
//
// Generates a minimal FDT (Flattened Device Tree) blob describing
// the virtual hardware: CPU, memory, UART, CLINT, PLIC, virtio-blk.
//
// The DTB is passed to the guest OS kernel in register a1 at boot.
// Format follows the Devicetree Specification v0.4.
//
// This is a hand-built DTB -- no external dependencies.
// We construct the binary blob directly using the FDT structure:
//   - Header (magic, totalsize, off_dt_struct, off_dt_strings, etc.)
//   - Structure block (tokens + nested nodes/properties)
//   - Strings block (null-terminated property names)

/// FDT magic number.
const FDT_MAGIC: u32 = 0xD00D_FEED;
/// FDT token: start of node.
const FDT_BEGIN_NODE: u32 = 0x0000_0001;
/// FDT token: end of node.
const FDT_END_NODE: u32 = 0x0000_0002;
/// FDT token: property.
const FDT_PROP: u32 = 0x0000_0003;
/// FDT token: end of structure block.
const FDT_END: u32 = 0x0000_0009;

/// DTB builder. Constructs an FDT blob in memory.
pub struct DtbBuilder {
    /// Structure block (tokens, nodes, properties).
    struct_block: Vec<u8>,
    /// Strings block (property names).
    strings_block: Vec<u8>,
    /// Memory reservation map entries: (address, size) pairs.
    /// Each entry is two u64 values (address, size), followed by a (0, 0) terminator.
    mem_rsvmap: Vec<(u64, u64)>,
}

impl Default for DtbBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DtbBuilder {
        /// Create a new DTB builder with empty struct and strings blocks.
    pub fn new() -> Self {
        Self {
            struct_block: Vec::new(),
            strings_block: Vec::new(),
            mem_rsvmap: Vec::new(),
        }
    }

    /// Add a string to the strings block, returning its offset.
    fn add_string(&mut self, s: &str) -> u32 {
        let offset = self.strings_block.len() as u32;
        self.strings_block.extend_from_slice(s.as_bytes());
        self.strings_block.push(0); // null terminator
        offset
    }

    /// Align the struct block to 4 bytes.
    fn align4(buf: &mut Vec<u8>) {
        while !buf.len().is_multiple_of(4) {
            buf.push(0);
        }
    }

    /// Add a memory reservation entry (address, size).
    /// These are processed by the kernel's early_init_fdt_scan() BEFORE setup_vm(),
    /// calling memblock_reserve() for each entry. This prevents memblock_alloc()
    /// from returning addresses in the reserved range.
    pub fn add_mem_reserve(&mut self, address: u64, size: u64) {
        self.mem_rsvmap.push((address, size));
    }

    /// Push a big-endian u32.
    fn push_u32(buf: &mut Vec<u8>, val: u32) {
        buf.extend_from_slice(&val.to_be_bytes());
    }

    fn push_u64(buf: &mut Vec<u8>, val: u64) {
        buf.extend_from_slice(&val.to_be_bytes());
    }

    /// Begin a new node.
    pub fn begin_node(&mut self, name: &str) {
        Self::push_u32(&mut self.struct_block, FDT_BEGIN_NODE);
        self.struct_block.extend_from_slice(name.as_bytes());
        self.struct_block.push(0); // null terminator
        Self::align4(&mut self.struct_block);
    }

    /// End the current node.
    pub fn end_node(&mut self) {
        Self::push_u32(&mut self.struct_block, FDT_END_NODE);
    }

    /// Add a property with a u32 value.
    pub fn prop_u32(&mut self, name: &str, val: u32) {
        let name_off = self.add_string(name);
        let data = val.to_be_bytes();
        Self::push_u32(&mut self.struct_block, FDT_PROP);
        Self::push_u32(&mut self.struct_block, data.len() as u32);
        Self::push_u32(&mut self.struct_block, name_off);
        self.struct_block.extend_from_slice(&data);
        Self::align4(&mut self.struct_block);
    }

    /// Add a property with a u64 value (as two u32 cells).
    pub fn prop_u64(&mut self, name: &str, val: u64) {
        let name_off = self.add_string(name);
        let data = val.to_be_bytes();
        Self::push_u32(&mut self.struct_block, FDT_PROP);
        Self::push_u32(&mut self.struct_block, data.len() as u32);
        Self::push_u32(&mut self.struct_block, name_off);
        self.struct_block.extend_from_slice(&data);
        Self::align4(&mut self.struct_block);
    }

    /// Add a property with a string value.
    pub fn prop_string(&mut self, name: &str, val: &str) {
        let name_off = self.add_string(name);
        Self::push_u32(&mut self.struct_block, FDT_PROP);
        Self::push_u32(&mut self.struct_block, (val.len() + 1) as u32);
        Self::push_u32(&mut self.struct_block, name_off);
        self.struct_block.extend_from_slice(val.as_bytes());
        self.struct_block.push(0); // null terminator
        Self::align4(&mut self.struct_block);
    }

    /// Add a property with raw bytes.
    pub fn prop_bytes(&mut self, name: &str, data: &[u8]) {
        let name_off = self.add_string(name);
        Self::push_u32(&mut self.struct_block, FDT_PROP);
        Self::push_u32(&mut self.struct_block, data.len() as u32);
        Self::push_u32(&mut self.struct_block, name_off);
        self.struct_block.extend_from_slice(data);
        Self::align4(&mut self.struct_block);
    }

    /// Add an empty property (exists but no value).
    pub fn prop_empty(&mut self, name: &str) {
        let name_off = self.add_string(name);
        Self::push_u32(&mut self.struct_block, FDT_PROP);
        Self::push_u32(&mut self.struct_block, 0);
        Self::push_u32(&mut self.struct_block, name_off);
    }

    /// Add a #address-cells and #size-cells pair.
    pub fn prop_cells(&mut self, addr_cells: u32, size_cells: u32) {
        self.prop_u32("#address-cells", addr_cells);
        self.prop_u32("#size-cells", size_cells);
    }

    /// Add a "reg" property with address and size (each as u64).
    pub fn prop_reg(&mut self, name: &str, addr: u64, size: u64) {
        let name_off = self.add_string(name);
        let mut data = Vec::new();
        data.extend_from_slice(&addr.to_be_bytes());
        data.extend_from_slice(&size.to_be_bytes());
        Self::push_u32(&mut self.struct_block, FDT_PROP);
        Self::push_u32(&mut self.struct_block, data.len() as u32);
        Self::push_u32(&mut self.struct_block, name_off);
        self.struct_block.extend_from_slice(&data);
        Self::align4(&mut self.struct_block);
    }

    /// Finish building and return the complete DTB blob.
    pub fn build(self) -> Vec<u8> {
        // Calculate sizes.
        let struct_size = self.struct_block.len() as u32;
        let strings_size = self.strings_block.len() as u32;

        // Memory reservation map: (address, size) pairs as u64 BE, terminated by (0, 0).
        // Each entry is 16 bytes. Terminator is 16 bytes.
        let mem_rsvmap_size = if self.mem_rsvmap.is_empty() {
            0u32
        } else {
            (self.mem_rsvmap.len() as u32 + 1) * 16 // +1 for terminator
        };

        // Header: 10 u32s = 40 bytes.
        let header_size: u32 = 40;
        let off_mem_rsvmap = header_size;
        let off_dt_struct = off_mem_rsvmap + mem_rsvmap_size;
        let off_dt_strings = off_dt_struct + struct_size;
        let totalsize = off_dt_strings + strings_size;

        let mut blob = Vec::with_capacity(totalsize as usize);

        // Header.
        Self::push_u32(&mut blob, FDT_MAGIC);
        Self::push_u32(&mut blob, totalsize);
        Self::push_u32(&mut blob, off_dt_struct);
        Self::push_u32(&mut blob, off_dt_strings);
        Self::push_u32(&mut blob, off_mem_rsvmap);
        Self::push_u32(&mut blob, header_size); // version
        Self::push_u32(&mut blob, header_size); // last_comp_version
        Self::push_u32(&mut blob, 0); // boot_cpuid_phys
        Self::push_u32(&mut blob, strings_size);
        Self::push_u32(&mut blob, struct_size);

        // Memory reservation map.
        for &(addr, size) in &self.mem_rsvmap {
            Self::push_u64(&mut blob, addr);
            Self::push_u64(&mut blob, size);
        }
        if !self.mem_rsvmap.is_empty() {
            // Terminator entry.
            Self::push_u64(&mut blob, 0);
            Self::push_u64(&mut blob, 0);
        }

        // Structure block (already has FDT_END appended by caller).
        blob.extend_from_slice(&self.struct_block);

        // Strings block.
        blob.extend_from_slice(&self.strings_block);

        blob
    }
}

/// Configuration for DTB generation.
pub struct DtbConfig {
    /// RAM base address.
    pub ram_base: u64,
    /// RAM size in bytes.
    pub ram_size: u64,
    /// Number of CPUs (default: 1).
    pub nr_cpus: u32,
    /// UART base address.
    pub uart_base: u64,
    /// CLINT base address.
    pub clint_base: u64,
    /// PLIC base address.
    pub plic_base: u64,
    /// Virtio MMIO base address.
    pub virtio_base: u64,
    /// Initrd start address (physical). None = no initrd.
    pub initrd_start: Option<u64>,
    /// Initrd end address (physical). None = no initrd.
    pub initrd_end: Option<u64>,
    /// Kernel boot command line.
    pub bootargs: String,
    /// Reserved memory regions: (base_address, size) pairs.
    /// The kernel's early DTB parser calls memblock_reserve() for these,
    /// preventing allocations from overlapping with kernel code/initramfs.
    pub reserved_regions: Vec<(u64, u64)>,
}

impl Default for DtbConfig {
    fn default() -> Self {
        Self {
            ram_base: 0x8000_0000,
            ram_size: 128 * 1024 * 1024, // 128MB
            nr_cpus: 1,
            uart_base: 0x1000_0000,
            clint_base: 0x0200_0000,
            plic_base: 0x0C00_0000,
            virtio_base: 0x1000_1000,
            initrd_start: None,
            initrd_end: None,
            bootargs: String::new(),
            reserved_regions: Vec::new(),
        }
    }
}

/// Generate a DTB for the RISC-V virtual machine.
pub fn generate_dtb(config: &DtbConfig) -> Vec<u8> {
    let mut b = DtbBuilder::new();

    // Memory reservation map entries.
    // These are processed by the kernel's early_init_fdt_scan_reserved_mem()
    // BEFORE setup_vm() allocates page tables. Each entry calls memblock_reserve(),
    // preventing memblock_alloc() from returning addresses in these ranges.
    // This is critical: without reserving PA 0..kernel_end, the kernel allocates
    // page tables at PA 0, overwriting its own code.
    for &(addr, size) in &config.reserved_regions {
        b.add_mem_reserve(addr, size);
    }

    // Root node.
    b.begin_node("");
    b.prop_u32("#address-cells", 2);
    b.prop_u32("#size-cells", 2);
    b.prop_string("compatible", "geometry-os,riscv-virt\0riscv-virtio");
    b.prop_string("model", "Geometry OS RISC-V Virtual Machine");

    // CPUs node.
    b.begin_node("cpus");
    b.prop_u32("#address-cells", 1);
    b.prop_u32("#size-cells", 0);
    b.prop_u32("timebase-frequency", 10_000_000);

    // CPU0.
    b.begin_node("cpu@0");
    b.prop_string("device_type", "cpu");
    b.prop_u32("reg", 0);
    b.prop_string("compatible", "riscv");
    b.prop_string("mmu-type", "riscv,sv32");
    b.prop_string("isa", "rv32imac");
    b.prop_string("status", "okay");
    // CPU interrupt controller (phandle 1).
    b.begin_node("interrupt-controller");
    b.prop_u32("#interrupt-cells", 1);
    b.prop_empty("interrupt-controller");
    b.prop_u32("phandle", 1);
    b.prop_string("compatible", "riscv,cpu-intc");
    b.end_node(); // interrupt-controller
    b.end_node(); // cpu@0

    b.end_node(); // cpus

    // Memory node. Name must match actual base address.
    let mem_name = format!("memory@{:x}", config.ram_base);
    b.begin_node(&mem_name);
    b.prop_string("device_type", "memory");
    b.prop_reg("reg", config.ram_base, config.ram_size);
    b.end_node();

    // Reserved memory node.
    // The kernel's early_init_fdt_scan_reserved_mem() parses this and calls
    // memblock_reserve() for each region BEFORE setup_vm() allocates page tables.
    // This prevents the allocator from returning PA 0 (which has kernel code).
    if !config.reserved_regions.is_empty() {
        b.begin_node("reserved-memory");
        b.prop_empty("ranges");
        for (i, &(base, size)) in config.reserved_regions.iter().enumerate() {
            let name = format!("region@{:x}", base);
            b.begin_node(&name);
            b.prop_string("compatible", "geometry-os,reserved\0reserved");
            b.prop_reg("reg", base, size);
            // NOTE: no-map removed -- the kernel needs these regions mapped
            // for the linear mapping. With kernel_map patched, __pa() returns
            // correct physical addresses, so the kernel's own page tables
            // will handle these regions correctly.
            b.end_node();
        }
        b.end_node();
    }

    // SOC node.
    b.begin_node("soc");
    b.prop_u32("#address-cells", 2);
    b.prop_u32("#size-cells", 2);
    b.prop_string("compatible", "simple-bus");
    b.prop_empty("ranges");

    // UART node.
    b.begin_node("uart@10000000");
    b.prop_string("compatible", "ns16550a");
    b.prop_reg("reg", config.uart_base, 0x100);
    b.prop_u32("interrupts", 10); // IRQ 10
    b.prop_u32("interrupt-parent", 2); // PLIC phandle
    b.prop_u32("reg-shift", 0); // Register stride: 1 byte apart (matches our UART emulation)
    b.prop_u32("reg-io-width", 1); // Use 8-bit (byte) I/O
    b.prop_u32("clock-frequency", 0); // Let driver use default
    b.end_node();

    // CLINT node.
    b.begin_node("clint@2000000");
    b.prop_string("compatible", "riscv,clint0");
    b.prop_reg("reg", config.clint_base, 0x10000);
    b.prop_u32("interrupts-extended", 1); // Reference CPU intc (phandle 1)
    b.end_node();

    // PLIC node (phandle 2).
    b.begin_node("plic@c000000");
    b.prop_string("compatible", "riscv,plic0");
    b.prop_reg("reg", config.plic_base, 0x200000);
    b.prop_empty("interrupt-controller");
    b.prop_u32("#interrupt-cells", 1);
    b.prop_u32("riscv,ndev", 31); // Number of external interrupt sources
    b.prop_u32("phandle", 2);
    b.prop_u32("interrupts-extended", 1); // Reference CPU intc (phandle 1)
    b.end_node();

    // Virtio MMIO node.
    b.begin_node("virtio@10001000");
    b.prop_string("compatible", "virtio,mmio");
    b.prop_reg("reg", config.virtio_base, 0x1000);
    b.prop_u32("interrupts", 1); // IRQ 1
    b.prop_u32("interrupt-parent", 2); // PLIC phandle
    b.end_node();

    b.end_node(); // soc

    // Chosen node (for boot args, initrd, etc).
    b.begin_node("chosen");
    b.prop_string("stdout-path", "/soc/uart@10000000");
    if let (Some(start), Some(end)) = (config.initrd_start, config.initrd_end) {
        // linux,initrd-start and linux,initrd-end are u64 values.
        b.prop_u64("linux,initrd-start", start);
        b.prop_u64("linux,initrd-end", end);
    }
    if !config.bootargs.is_empty() {
        b.prop_string("bootargs", &config.bootargs);
    }
    b.end_node();

    b.end_node(); // root

    // End token.
    DtbBuilder::push_u32(&mut b.struct_block, FDT_END);

    b.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtb_starts_with_magic() {
        let dtb = generate_dtb(&DtbConfig::default());
        let magic = u32::from_be_bytes([dtb[0], dtb[1], dtb[2], dtb[3]]);
        assert_eq!(magic, FDT_MAGIC);
    }

    #[test]
    fn dtb_totalsize_matches_blob_len() {
        let dtb = generate_dtb(&DtbConfig::default());
        let totalsize = u32::from_be_bytes([dtb[4], dtb[5], dtb[6], dtb[7]]);
        assert_eq!(totalsize as usize, dtb.len());
    }

    #[test]
    fn dtb_has_valid_structure_offset() {
        let dtb = generate_dtb(&DtbConfig::default());
        let off_struct = u32::from_be_bytes([dtb[8], dtb[9], dtb[10], dtb[11]]);
        // First token should be FDT_BEGIN_NODE at the structure offset.
        let token = u32::from_be_bytes([
            dtb[off_struct as usize],
            dtb[off_struct as usize + 1],
            dtb[off_struct as usize + 2],
            dtb[off_struct as usize + 3],
        ]);
        assert_eq!(token, FDT_BEGIN_NODE);
    }

    #[test]
    fn dtb_contains_strings() {
        let dtb = generate_dtb(&DtbConfig::default());
        let off_strings = u32::from_be_bytes([dtb[12], dtb[13], dtb[14], dtb[15]]);
        let strings_start = off_strings as usize;
        // Strings block should contain "compatible" at least.
        let strings = &dtb[strings_start..];
        let s = std::str::from_utf8(strings).unwrap_or("");
        assert!(s.contains("compatible"));
    }

    #[test]
    fn dtb_custom_ram_size() {
        let config = DtbConfig {
            ram_size: 256 * 1024 * 1024,
            ..Default::default()
        };
        let dtb = generate_dtb(&config);
        // Should still be valid.
        let magic = u32::from_be_bytes([dtb[0], dtb[1], dtb[2], dtb[3]]);
        assert_eq!(magic, FDT_MAGIC);
        assert!(dtb.len() > 40);
    }

    #[test]
    fn dtb_minimal_config() {
        let config = DtbConfig {
            ram_base: 0x8000_0000,
            ram_size: 4096,
            nr_cpus: 1,
            uart_base: 0x1000_0000,
            clint_base: 0x0200_0000,
            plic_base: 0x0C00_0000,
            virtio_base: 0x1000_1000,
            initrd_start: None,
            initrd_end: None,
            bootargs: String::new(),
            reserved_regions: Vec::new(),
        };
        let dtb = generate_dtb(&config);
        let magic = u32::from_be_bytes([dtb[0], dtb[1], dtb[2], dtb[3]]);
        assert_eq!(magic, FDT_MAGIC);
    }
}
