// Bus-attached device interface.

use machina_core::address::GPA;
use machina_memory::region::MemoryRegion;

use crate::qdev::Device;

/// A device that can be attached to a memory-mapped bus.
pub trait BusDevice: Device {
    fn read(&self, offset: u64, size: u32) -> u64;
    fn write(&mut self, offset: u64, size: u32, val: u64);
}

// -- SysBus --------------------------------------------------------

/// One region mapped at a fixed guest-physical address.
pub struct SysBusMapping {
    pub region: MemoryRegion,
    pub base: GPA,
}

/// System bus — the default bus for platform devices.
///
/// Holds a list of memory-region mappings that the board
/// wiring code populates during machine init.
pub struct SysBus {
    pub name: String,
    mappings: Vec<SysBusMapping>,
}

impl SysBus {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            mappings: Vec::new(),
        }
    }

    /// Register a memory region at `base`.
    pub fn add_mapping(&mut self, region: MemoryRegion, base: GPA) {
        self.mappings.push(SysBusMapping { region, base });
    }

    /// Read-only view of all registered mappings.
    pub fn mappings(&self) -> &[SysBusMapping] {
        &self.mappings
    }
}
