use serde::Serialize;

use crate::{config::get_config, define_type_enum};

use super::Ship;

define_type_enum!(ShipUpgrade {
    CargoExpansion,
    Reactor,
    HullUpgrade,
    Shield,
});

impl ShipUpgrade {
    pub fn get_price(&self) -> f64 {
        let cfg = get_config();
        match self {
            ShipUpgrade::CargoExpansion => {
                cfg.upgrade.ship_cargo_exp_add_cap * cfg.ship.cargo_cap_price * 1.0
            }
            ShipUpgrade::Reactor => cfg.ship.reactor_power_price * 1.0,
            ShipUpgrade::HullUpgrade => {
                cfg.upgrade.ship_hull_upg_add * cfg.ship.hull_resis_price * 1.0
            }
            ShipUpgrade::Shield => cfg.ship.shield_price * 1.0,
        }
    }

    pub fn install(&self, ship: &mut Ship) {
        let cfg = get_config();
        match self {
            ShipUpgrade::CargoExpansion => {
                ship.cargo.capacity += cfg.upgrade.ship_cargo_exp_add_cap
            }
            ShipUpgrade::HullUpgrade => ship.hull_resistance += cfg.upgrade.ship_hull_upg_add,
            ShipUpgrade::Reactor => ship.reactor_power += 1,
            ShipUpgrade::Shield => ship.shield_power += 1,
        }
        ship.update_perf_stats();
    }
}
