use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumString, IntoStaticStr};

use crate::galaxy::planet::Planet;

use super::{cargo::ShipCargo, Ship};

#[derive(
    EnumIter,
    EnumString,
    IntoStaticStr,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Clone,
    Copy,
)]
#[strum(ascii_case_insensitive)]
pub enum Resource {
    // Solid
    Carbon,
    Iron,
    Copper,
    Gold,

    // Liquid
    Water,
    Alcohol,
    Oil,
    SulfuricAcid,

    // Gaseous
    Hydrogen,
    Oxygen,
    Helium,
    Ozone,

    // Crafted
    Fuel,
    Hull,
}

impl Resource {
    pub fn scored(&self) -> bool {
        matches!(self, Resource::Fuel | Resource::Hull)
    }

    #[inline]
    pub const fn base_price(&self) -> f64 {
        let base = 4.0;
        match self {
            Resource::Carbon | Resource::Hydrogen | Resource::Water => base,
            Resource::Iron | Resource::Oxygen | Resource::Alcohol => 4.0 * base,
            Resource::Copper | Resource::Helium | Resource::Oil => 12.0 * base,
            Resource::Gold | Resource::Ozone | Resource::SulfuricAcid => 16.0 * base,
            Resource::Fuel | Resource::Hull => base / 2.0,
        }
    }

    pub fn volume(&self) -> f64 {
        match self {
            Resource::Carbon | Resource::Hydrogen | Resource::Water => 0.75,
            Resource::Iron | Resource::Oxygen | Resource::Alcohol => 2.5,
            Resource::Copper | Resource::Helium | Resource::Oil => 3.0,
            Resource::Gold | Resource::Ozone | Resource::SulfuricAcid => 0.25,
            Resource::Fuel => 2.0,
            Resource::Hull => 0.05,
        }
    }

    #[allow(unused_mut)]
    pub fn extraction_difficulty(&self) -> f64 {
        let mut base = 0.25;

        #[cfg(feature = "extraspeed")]
        {
            base /= 1000.0;
        }
        match self {
            Resource::Carbon | Resource::Hydrogen => base,
            Resource::Iron | Resource::Oxygen => 3.75 * base,
            Resource::Copper | Resource::Helium => 11.0 * base,
            Resource::Gold | Resource::Ozone => 14.0 * base,

            // All the things that are only crafted
            _ => unreachable!("Extraction difficulty on crafted resources"),
        }
    }

    pub fn min_rank(&self) -> u8 {
        match self {
            Resource::Carbon | Resource::Hydrogen | Resource::Water => 0,
            Resource::Iron | Resource::Oxygen | Resource::Alcohol => 2,
            Resource::Copper | Resource::Helium | Resource::Oil => 4,
            Resource::Gold | Resource::Ozone | Resource::SulfuricAcid => 6,
            Resource::Fuel | Resource::Hull => 0,
        }
    }

    pub fn mineable(&self, rank: u8) -> bool {
        match self {
            Resource::Carbon | Resource::Iron | Resource::Copper | Resource::Gold => {
                rank > self.min_rank()
            }
            _ => false,
        }
    }

    pub fn suckable(&self, rank: u8) -> bool {
        match self {
            Resource::Hydrogen | Resource::Oxygen | Resource::Helium | Resource::Ozone => {
                rank > self.min_rank()
            }
            _ => false,
        }
    }

    pub fn pumpable(&self, rank: u8) -> bool {
        match self {
            Resource::Water | Resource::Alcohol | Resource::Oil | Resource::SulfuricAcid => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    const MINEABLE: [Resource; 4] = [
        Resource::Carbon,
        Resource::Iron,
        Resource::Copper,
        Resource::Gold,
    ];
    const SUCKABLE: [Resource; 4] = [
        Resource::Hydrogen,
        Resource::Oxygen,
        Resource::Helium,
        Resource::Ozone,
    ];
    const PUMPABLE: [Resource; 4] = [
        Resource::Water,
        Resource::Alcohol,
        Resource::Oil,
        Resource::SulfuricAcid,
    ];
    // Resources for which extraction_difficulty() is defined (not crafted/liquid)
    const EXTRACTABLE: [Resource; 8] = [
        Resource::Carbon,
        Resource::Hydrogen,
        Resource::Iron,
        Resource::Oxygen,
        Resource::Copper,
        Resource::Helium,
        Resource::Gold,
        Resource::Ozone,
    ];

    #[test]
    fn test_scored_only_fuel_and_hull() {
        for r in Resource::iter() {
            let expected = matches!(r, Resource::Fuel | Resource::Hull);
            assert_eq!(r.scored(), expected, "scored mismatch for {r:?}");
        }
    }

    #[test]
    fn test_base_price_positive_for_all() {
        for r in Resource::iter() {
            assert!(r.base_price() > 0.0, "base_price <= 0 for {r:?}");
        }
        assert_eq!(Resource::Carbon.base_price(), 4.0);
        assert_eq!(Resource::Iron.base_price(), 16.0);
        assert_eq!(Resource::Gold.base_price(), 64.0);
        assert_eq!(Resource::Fuel.base_price(), 2.0);
        assert_eq!(Resource::Hull.base_price(), 2.0);
    }

    #[test]
    fn test_volume_positive_for_all() {
        for r in Resource::iter() {
            assert!(r.volume() > 0.0, "volume <= 0 for {r:?}");
        }
        assert_eq!(Resource::Carbon.volume(), 0.75);
        assert_eq!(Resource::Hull.volume(), 0.05);
    }

    #[test]
    fn test_min_rank_values() {
        assert_eq!(Resource::Carbon.min_rank(), 0);
        assert_eq!(Resource::Iron.min_rank(), 2);
        assert_eq!(Resource::Copper.min_rank(), 4);
        assert_eq!(Resource::Gold.min_rank(), 6);
        assert_eq!(Resource::Fuel.min_rank(), 0);
    }

    #[test]
    fn test_extraction_difficulty_positive() {
        for r in EXTRACTABLE {
            assert!(r.extraction_difficulty() > 0.0, "difficulty <= 0 for {r:?}");
        }
        // Higher tier resources are harder to extract
        assert!(Resource::Gold.extraction_difficulty() > Resource::Carbon.extraction_difficulty());
        assert!(Resource::Copper.extraction_difficulty() > Resource::Iron.extraction_difficulty());
    }

    #[test]
    fn test_mineable_classification() {
        for r in MINEABLE {
            assert!(r.mineable(u8::MAX), "{r:?} should be mineable at max rank");
            // Not mineable at or below the min rank
            assert!(!r.mineable(r.min_rank()), "{r:?} mineable at min rank");
        }
        // Non-mineable categories are never mineable
        for r in SUCKABLE.into_iter().chain(PUMPABLE) {
            assert!(!r.mineable(u8::MAX), "{r:?} should not be mineable");
        }
        assert!(!Resource::Fuel.mineable(u8::MAX));
    }

    #[test]
    fn test_suckable_classification() {
        for r in SUCKABLE {
            assert!(r.suckable(u8::MAX), "{r:?} should be suckable");
            assert!(!r.suckable(r.min_rank()), "{r:?} suckable at min rank");
        }
        for r in MINEABLE.into_iter().chain(PUMPABLE) {
            assert!(!r.suckable(u8::MAX), "{r:?} should not be suckable");
        }
    }

    #[test]
    fn test_pumpable_classification() {
        for r in PUMPABLE {
            assert!(r.pumpable(u8::MAX), "{r:?} should be pumpable");
            assert!(!r.pumpable(r.min_rank()), "{r:?} pumpable at min rank");
        }
        for r in MINEABLE.into_iter().chain(SUCKABLE) {
            assert!(!r.pumpable(u8::MAX), "{r:?} should not be pumpable");
        }
    }

    #[test]
    fn test_extraction_categories_are_disjoint() {
        // No resource is in more than one extraction category at once
        for r in Resource::iter() {
            let n = [
                r.mineable(u8::MAX),
                r.suckable(u8::MAX),
                r.pumpable(u8::MAX),
            ]
            .into_iter()
            .filter(|b| *b)
            .count();
            assert!(n <= 1, "{r:?} belongs to several extraction categories");
        }
    }
}
