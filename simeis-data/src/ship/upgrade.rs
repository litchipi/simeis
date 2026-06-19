use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumString, IntoStaticStr};

use super::{Ship, CARGO_CAP_PRICE, HULL_RESIS_PRICE, REACTOR_POWER_PRICE, SHIELD_PRICE};

const CARGO_EXP_ADD_CAP: f64 = 120.0;
const REACTOR_UPG_ADD: u16 = 1;
const HULL_UPG_ADD: f64 = 100.0;
const SHIELD_UPG_ADD: u16 = 1;

const REACTOR_OPT_DEC_FUELCONS: f64 = 5.0 / 100.0;
const REACTOR_OPT_PRICE: f64 = 1000.0;

#[derive(
    EnumIter,
    EnumString,
    IntoStaticStr,
    Debug,
    Serialize,
    Deserialize,
    Ord,
    PartialOrd,
    PartialEq,
    Eq,
    Clone,
    Copy,
)]
#[strum(ascii_case_insensitive)]
pub enum ShipUpgrade {
    CargoExpansion,
    ReactorUpgrade,
    HullUpgrade,
    Shield,
}

impl ShipUpgrade {
    pub fn get_price(&self) -> f64 {
        match self {
            ShipUpgrade::CargoExpansion => CARGO_EXP_ADD_CAP * CARGO_CAP_PRICE * 1.0,
            ShipUpgrade::ReactorUpgrade => (REACTOR_UPG_ADD as f64) * REACTOR_POWER_PRICE * 1.0,
            ShipUpgrade::HullUpgrade => HULL_UPG_ADD * HULL_RESIS_PRICE * 1.0,
            ShipUpgrade::Shield => (SHIELD_UPG_ADD as f64) * SHIELD_PRICE * 1.0,
        }
    }

    pub fn install(&self, ship: &mut Ship) {
        match self {
            ShipUpgrade::CargoExpansion => ship.cargo.capacity += CARGO_EXP_ADD_CAP,
            ShipUpgrade::ReactorUpgrade => ship.reactor_power += REACTOR_UPG_ADD,
            ShipUpgrade::HullUpgrade => ship.hull_resistance += HULL_UPG_ADD,
            ShipUpgrade::Shield => ship.shield_power += SHIELD_UPG_ADD,
        }
        ship.update_perf_stats();
    }

    pub fn description(&self) -> String {
        match self {
            ShipUpgrade::CargoExpansion => format!("Adds {CARGO_EXP_ADD_CAP} of cargo capacity"),
            ShipUpgrade::ReactorUpgrade => format!(
                "Increase the reactor power by {REACTOR_UPG_ADD}, improves the ship's speed"
            ),
            ShipUpgrade::HullUpgrade => {
                format!("Increase the hull decay capacity by {HULL_UPG_ADD}")
            }
            ShipUpgrade::Shield => "Reduce the damage and usure of the hull".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_all_upgrades_have_positive_price_and_description() {
        for u in ShipUpgrade::iter() {
            assert!(u.get_price() > 0.0, "price <= 0 for {u:?}");
            assert!(!u.description().is_empty(), "empty description for {u:?}");
        }
    }

    #[test]
    fn test_cargo_expansion_increases_capacity() {
        let mut ship = Ship::default();
        let before = ship.cargo.capacity;
        ShipUpgrade::CargoExpansion.install(&mut ship);
        assert!(ship.cargo.capacity > before);
    }

    #[test]
    fn test_reactor_upgrade_increases_reactor_power() {
        let mut ship = Ship::default();
        let before = ship.reactor_power;
        ShipUpgrade::ReactorUpgrade.install(&mut ship);
        assert_eq!(ship.reactor_power, before + REACTOR_UPG_ADD);
    }

    #[test]
    fn test_hull_upgrade_increases_resistance() {
        let mut ship = Ship::default();
        let before = ship.hull_resistance;
        ShipUpgrade::HullUpgrade.install(&mut ship);
        assert!((ship.hull_resistance - (before + HULL_UPG_ADD)).abs() < 1e-9);
    }

    #[test]
    fn test_shield_upgrade_increases_shield_power() {
        let mut ship = Ship::default();
        let before = ship.shield_power;
        ShipUpgrade::Shield.install(&mut ship);
        assert_eq!(ship.shield_power, before + SHIELD_UPG_ADD);
    }
}
