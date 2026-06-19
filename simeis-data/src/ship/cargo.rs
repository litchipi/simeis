use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::resources::Resource;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShipCargo {
    pub capacity: f64,
    pub usage: f64,
    pub resources: BTreeMap<Resource, f64>,
}

impl ShipCargo {
    pub const fn with_capacity(cap: f64) -> ShipCargo {
        ShipCargo {
            usage: 0.0,
            capacity: cap,
            resources: BTreeMap::new(),
        }
    }

    pub fn slowing_ratio(&self) -> f64 {
        // let usage_ratio = self.usage / self.capacity;
        0.0
    }

    pub fn add_resource(&mut self, res: &Resource, mut amnt: f64) -> f64 {
        let added = res.volume() * amnt;
        if self.usage == self.capacity {
            return 0.0;
        } else if (self.usage + added) > self.capacity {
            let overflow = (self.usage + added) - self.capacity;
            amnt -= overflow / res.volume();
            self.usage = self.capacity;
        } else {
            self.usage += added;
        }

        if let Some(stock) = self.resources.get_mut(res) {
            *stock += amnt;
        } else {
            self.resources.insert(*res, amnt);
        }
        amnt
    }

    pub fn is_full(&self) -> bool {
        self.usage == self.capacity
    }

    pub fn unload(&mut self, resource: &Resource, amnt: f64) -> f64 {
        if let Some(got) = self.resources.get_mut(resource) {
            let unload = got.min(amnt);
            *got -= unload;
            self.usage = (self.usage - (resource.volume() * unload)).max(0.0);
            self.usage = (self.usage * 1000.0).round() / 1000.0;
            unload
        } else {
            0.0
        }
    }

    // Compute how much of a resource we can store (based on its volume)
    pub fn space_for(&self, resource: &Resource) -> f64 {
        let capleft = self.capacity - self.usage;
        capleft / resource.volume()
    }
}

#[test]
fn test_cargo_overflow() {
    let mut cargo = ShipCargo::with_capacity(100.0 * Resource::Carbon.volume());
    let added = cargo.add_resource(&Resource::Carbon, 95.0);
    assert_eq!(added, 95.0);

    let added = cargo.add_resource(&Resource::Carbon, 10.0);
    assert_eq!(added, 5.0);
    assert_eq!(cargo.usage, cargo.capacity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_capacity_starts_empty() {
        let cargo = ShipCargo::with_capacity(500.0);
        assert_eq!(cargo.capacity, 500.0);
        assert_eq!(cargo.usage, 0.0);
        assert!(cargo.resources.is_empty());
        assert!(!cargo.is_full());
    }

    #[test]
    fn test_add_resource_updates_usage_by_volume() {
        let mut cargo = ShipCargo::with_capacity(1000.0);
        let added = cargo.add_resource(&Resource::Carbon, 10.0);
        assert_eq!(added, 10.0);
        assert!((cargo.usage - 10.0 * Resource::Carbon.volume()).abs() < 1e-9);
        assert_eq!(cargo.resources[&Resource::Carbon], 10.0);
    }

    #[test]
    fn test_add_resource_accumulates_same_resource() {
        let mut cargo = ShipCargo::with_capacity(1000.0);
        cargo.add_resource(&Resource::Iron, 5.0);
        cargo.add_resource(&Resource::Iron, 3.0);
        assert_eq!(cargo.resources[&Resource::Iron], 8.0);
    }

    #[test]
    fn test_add_to_full_cargo_returns_zero() {
        let mut cargo = ShipCargo::with_capacity(10.0 * Resource::Carbon.volume());
        cargo.add_resource(&Resource::Carbon, 10.0);
        assert!(cargo.is_full());
        let added = cargo.add_resource(&Resource::Carbon, 5.0);
        assert_eq!(added, 0.0);
    }

    #[test]
    fn test_unload_reduces_usage() {
        let mut cargo = ShipCargo::with_capacity(1000.0);
        cargo.add_resource(&Resource::Copper, 20.0);
        let unloaded = cargo.unload(&Resource::Copper, 8.0);
        assert_eq!(unloaded, 8.0);
        assert_eq!(cargo.resources[&Resource::Copper], 12.0);
    }

    #[test]
    fn test_unload_more_than_present_clamps() {
        let mut cargo = ShipCargo::with_capacity(1000.0);
        cargo.add_resource(&Resource::Gold, 4.0);
        let unloaded = cargo.unload(&Resource::Gold, 100.0);
        assert_eq!(unloaded, 4.0);
        assert_eq!(cargo.resources[&Resource::Gold], 0.0);
    }

    #[test]
    fn test_unload_missing_resource_returns_zero() {
        let mut cargo = ShipCargo::with_capacity(1000.0);
        assert_eq!(cargo.unload(&Resource::Water, 5.0), 0.0);
    }

    #[test]
    fn test_space_for_decreases_after_adding() {
        let mut cargo = ShipCargo::with_capacity(100.0);
        let before = cargo.space_for(&Resource::Carbon);
        cargo.add_resource(&Resource::Carbon, 10.0);
        let after = cargo.space_for(&Resource::Carbon);
        assert!(after < before);
        assert!((before - 100.0 / Resource::Carbon.volume()).abs() < 1e-9);
    }

    #[test]
    fn test_slowing_ratio_is_zero() {
        let cargo = ShipCargo::with_capacity(100.0);
        assert_eq!(cargo.slowing_ratio(), 0.0);
    }
}
