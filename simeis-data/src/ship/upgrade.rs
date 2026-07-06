use serde::Serialize;

use crate::{config::get_config, define_type_enum};

use super::Ship;

define_type_enum!(ShipUpgrade {
    Reactor,
    Shield,
});

impl ShipUpgrade {
    pub fn get_price(&self) -> f64 {
        let cfg = get_config();
        match self {
            ShipUpgrade::Reactor => cfg.ship.reactor_rank_price * 1.0,
            ShipUpgrade::Shield => cfg.ship.shield_price * 1.0,
        }
    }

    pub fn install(&self, ship: &mut Ship) {
        match self {
            ShipUpgrade::Reactor => ship.reactor_rank += 1,
            ShipUpgrade::Shield => ship.shield_rank += 1,
        }
        ship.update_perf_stats();
    }
}

#[derive(strum::EnumString, strum::Display, strum::EnumIter, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum ShipExpandUpgrade {
    CargoExpansion(f64),
    HullExpansion(f64),
}

impl ShipExpandUpgrade {
    pub fn set_amount(&mut self, amnt: f64) {
        match self {
            ShipExpandUpgrade::CargoExpansion(n)
            | ShipExpandUpgrade::HullExpansion(n) => *n = amnt,
        }
    }
    pub fn get_price(&self) -> f64 {
        let cfg = get_config();
        match self {
            ShipExpandUpgrade::CargoExpansion(n) => n * cfg.ship.cargo_cap_price,
            ShipExpandUpgrade::HullExpansion(n) => n * cfg.ship.hull_resis_price,
        }
    }

    pub fn install(&self, ship: &mut Ship) {
        match self {
            ShipExpandUpgrade::CargoExpansion(n) => ship.cargo.capacity += n,
            ShipExpandUpgrade::HullExpansion(n) => ship.hull_resistance += n,
        }
        ship.update_perf_stats();
    }
}
