use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::define_type_enum;
use crate::galaxy::planet::Planet;
use crate::ship::module::ShipModuleType;
use crate::ship::{cargo::ShipCargo, Ship};

define_type_enum!(Resource {
    // Solid
    Iron,
    Copper,
    Titanium,
    Uranium,

    // Liquid
    Water,
    Ammonia,
    SulfuricAcid,
    Brine,

    // Gaseous
    Hydrogen,
    Oxygen,
    Helium,
    Ozone,

    // Crafted
    Fuel,
    Hull,
});

impl Resource {
    pub fn scored(&self) -> bool {
        matches!(self, Resource::Fuel | Resource::Hull)
    }

    // TODO (#12) Get from configuration file
    #[inline]
    pub const fn base_price(&self) -> f64 {
        let base = 4.0;
        match self {
            Resource::Iron | Resource::Hydrogen | Resource::Water => base,
            Resource::Copper | Resource::Oxygen | Resource::Ammonia => 4.0 * base,
            Resource::Titanium | Resource::Helium | Resource::SulfuricAcid => 12.0 * base,
            Resource::Uranium | Resource::Ozone | Resource::Brine => 16.0 * base,
            Resource::Fuel | Resource::Hull => base / 2.0,
        }
    }

    #[inline]
    pub fn volume(&self) -> f64 {
        match self {
            Resource::Iron | Resource::Hydrogen | Resource::Water => 0.75,
            Resource::Copper | Resource::Oxygen | Resource::Ammonia => 2.5,
            Resource::Titanium | Resource::Helium | Resource::SulfuricAcid => 3.0,
            Resource::Uranium | Resource::Ozone | Resource::Brine => 0.25,
            Resource::Fuel => 2.0,
            Resource::Hull => 0.05,
        }
    }

    #[allow(unused_mut)]
    #[inline]
    pub fn extraction_difficulty(&self) -> f64 {
        let mut base = 1.0;

        #[cfg(feature = "extraspeed")]
        {
            base /= 1000.0;
        }
        match self {
            Resource::Iron | Resource::Hydrogen | Resource::Water => base,
            Resource::Copper | Resource::Oxygen | Resource::Ammonia => 3.75 * base,
            Resource::Titanium | Resource::Helium | Resource::SulfuricAcid => 11.0 * base,
            Resource::Uranium | Resource::Ozone | Resource::Brine => 14.0 * base,

            // All the things that are only crafted
            _ => unreachable!("Extraction difficulty on crafted resources"),
        }
    }

    #[inline]
    pub fn min_rank(&self) -> u8 {
        match self {
            Resource::Iron | Resource::Hydrogen | Resource::Water => 0,
            Resource::Copper | Resource::Oxygen | Resource::Ammonia => 2,
            Resource::Titanium | Resource::Helium | Resource::SulfuricAcid => 4,
            Resource::Uranium | Resource::Ozone | Resource::Brine => 6,
            Resource::Fuel | Resource::Hull => 0,
        }
    }

    #[inline]
    pub fn extractable(&self, modtype: &ShipModuleType, oprank: u8) -> bool {
        match modtype {
            ShipModuleType::Miner => self.mineable(oprank),
            ShipModuleType::GasSucker => self.suckable(oprank),
            ShipModuleType::Pump => self.pumpable(oprank),
        }
    }

    #[inline]
    pub fn mineable(&self, rank: u8) -> bool {
        match self {
            Resource::Iron | Resource::Copper | Resource::Titanium | Resource::Uranium => {
                rank > self.min_rank()
            }
            _ => false,
        }
    }

    #[inline]
    pub fn suckable(&self, rank: u8) -> bool {
        match self {
            Resource::Hydrogen | Resource::Oxygen | Resource::Helium | Resource::Ozone => {
                rank > self.min_rank()
            }
            _ => false,
        }
    }

    #[inline]
    pub fn pumpable(&self, rank: u8) -> bool {
        match self {
            Resource::Water | Resource::Ammonia | Resource::SulfuricAcid | Resource::Brine => {
                rank > self.min_rank()
            }
            _ => false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ExtractionInfo {
    pub mining_rate: BTreeMap<Resource, f64>,
    pub time_fill_cargo: f64,
}

impl ExtractionInfo {
    pub fn create(ship: &Ship, planet: &Planet) -> Self {
        let mut extraction = BTreeMap::new();
        let mut cap_per_sec = 0.0;
        for (_, smod) in ship.modules.iter() {
            for (res, rate) in smod.can_extract(&ship.crew, planet) {
                cap_per_sec += rate * res.volume();
                if let Some(rrate) = extraction.get_mut(&res) {
                    *rrate += rate;
                } else {
                    extraction.insert(res, rate);
                }
            }
        }
        apply_power_attribution(&ship.power_attribution, &mut extraction);
        ExtractionInfo {
            mining_rate: extraction,
            time_fill_cargo: ship.cargo.capacity / cap_per_sec,
        }
    }

    pub fn update_cargo(&self, cargo: &mut ShipCargo, tdelta: f64) -> bool {
        for (res, rate) in self.mining_rate.iter() {
            cargo.add_resource(res, *rate * tdelta);
        }
        cargo.is_full()
    }
}

pub fn apply_power_attribution(attrs: &BTreeMap<Resource, u8>, r: &mut BTreeMap<Resource, f64>) {
    let powtot: u16 = r
        .keys()
        .map(|r| *attrs.get(r).unwrap_or(&u8::MAX) as u16)
        .sum();
    r.iter_mut().for_each(|(res, rate)| {
        let power = attrs.get(res).unwrap_or(&u8::MAX);
        let power_f = (*power as f64) / (powtot as f64);
        *rate *= power_f;
    });
}
