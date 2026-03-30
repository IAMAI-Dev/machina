// Device object model — analogous to QEMU hw/core/qdev.c

use std::any::Any;

/// Base trait for all emulated devices.
pub trait Device: Send + Sync {
    fn name(&self) -> &str;
    fn realize(&mut self) -> Result<(), String>;
    fn reset(&mut self);
    fn realized(&self) -> bool;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Common state shared by every device instance.
pub struct DeviceState {
    pub name: String,
    pub realized: bool,
    parent_bus: Option<String>,
}

impl DeviceState {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            realized: false,
            parent_bus: None,
        }
    }

    /// Attach this device to the named bus.
    pub fn set_parent_bus(&mut self, bus: &str) {
        self.parent_bus = Some(bus.to_string());
    }

    /// Return the parent bus name, if any.
    pub fn parent_bus(&self) -> Option<&str> {
        self.parent_bus.as_deref()
    }
}
