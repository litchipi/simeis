use serde::Serialize;

use crate::define_type_enum;

use super::{Ship, CARGO_CAP_PRICE, HULL_RESIS_PRICE, REACTOR_POWER_PRICE, SHIELD_PRICE};

const CARGO_EXP_ADD_CAP: f64 = 120.0;
const HULL_UPG_ADD: f64 = 100.0;

define_type_enum!(ShipUpgrade {
    CargoExpansion,
    Reactor,
    HullUpgrade,
    Shield,
});

impl ShipUpgrade {
    pub fn get_price(&self) -> f64 {
        match self {
            ShipUpgrade::CargoExpansion => CARGO_EXP_ADD_CAP * CARGO_CAP_PRICE * 1.0,
            ShipUpgrade::Reactor => REACTOR_POWER_PRICE * 1.0,
            ShipUpgrade::HullUpgrade => HULL_UPG_ADD * HULL_RESIS_PRICE * 1.0,
            ShipUpgrade::Shield => SHIELD_PRICE * 1.0,
        }
    }

    pub fn install(&self, ship: &mut Ship) {
        match self {
            ShipUpgrade::CargoExpansion => ship.cargo.capacity += CARGO_EXP_ADD_CAP,
            ShipUpgrade::HullUpgrade => ship.hull_resistance += HULL_UPG_ADD,
            ShipUpgrade::Reactor => ship.reactor_power += 1,
            ShipUpgrade::Shield => ship.shield_power += 1,
        }
        ship.update_perf_stats();
    }

    pub fn description(&self) -> String {
        match self {
            ShipUpgrade::CargoExpansion => format!("Adds {CARGO_EXP_ADD_CAP} of cargo capacity"),
            ShipUpgrade::Reactor => {
                "Increase the reactor power by 1, improves the ship's speed".to_string()
            }
            ShipUpgrade::HullUpgrade => {
                format!("Increase the hull decay capacity by {HULL_UPG_ADD}")
            }
            ShipUpgrade::Shield => {
                "Increase the shield power by 1, reduces the damage and usure taken by the hull"
                    .to_string()
            }
        }
    }
}
