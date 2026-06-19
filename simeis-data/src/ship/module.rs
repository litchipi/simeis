use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};

use super::resources::Resource;
use crate::crew::{Crew, CrewId, CrewMemberType};
use crate::galaxy::planet::Planet;

const MOD_UPG_POWF_DIV: f64 = 75.0;
const EXTRACTION_RATE_RANK_POWF: f64 = 0.45;
const EXRATE_DIFF_FACT: f64 = 2.5;
const EXRATE_FACT: f64 = 0.6;

pub type ShipModuleId = u16;

#[derive(
    EnumIter,
    EnumString,
    IntoStaticStr,
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
#[strum(ascii_case_insensitive)]
pub enum ShipModuleType {
    Miner,
    GasSucker,
    Pump,
}

impl ShipModuleType {
    pub fn new_module(self) -> ShipModule {
        ShipModule {
            operator: None,
            modtype: self,
            totalcost: 0.0,
            rank: 1,
        }
    }

    #[inline]
    pub fn get_price_buy(&self) -> f64 {
        match self {
            ShipModuleType::Miner | ShipModuleType::Pump | ShipModuleType::GasSucker => 4500.0,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ShipModule {
    pub operator: Option<CrewId>,
    pub modtype: ShipModuleType,
    pub rank: u8,
    pub totalcost: f64,
}

impl ShipModule {
    #[inline]
    pub fn price_next_rank(&self) -> f64 {
        let num = MOD_UPG_POWF_DIV - 1.0 + (self.rank as f64);
        self.modtype.get_price_buy().powf(num / MOD_UPG_POWF_DIV)
    }

    // Returns
    pub fn need(&self, ctype: &CrewMemberType) -> bool {
        match self.modtype {
            ShipModuleType::Miner | ShipModuleType::Pump | ShipModuleType::GasSucker => {
                ctype == &CrewMemberType::Operator && self.operator.is_none()
            }
        }
    }

    pub fn can_extract(&self, crew: &Crew, planet: &Planet) -> Vec<(Resource, f64)> {
        let Some(ref opid) = self.operator else {
            log::debug!("No operator");
            return vec![];
        };

        let cm = crew.0.get(opid).unwrap();
        let all_resources = Resource::iter()
            .map(|r| (r, planet.resource_density(&r)))
            .filter(|(_, d)| *d > 0.0);

        match self.modtype {
            ShipModuleType::Miner => all_resources
                .filter(|(r, _)| r.mineable(cm.rank))
                .map(|(r, density)| (r, self.extraction_rate(&r, cm.rank, density)))
                .collect(),
            ShipModuleType::GasSucker => all_resources
                .filter(|(r, _)| r.suckable(cm.rank))
                .map(|(r, density)| (r, self.extraction_rate(&r, cm.rank, density)))
                .collect(),
            ShipModuleType::Pump => all_resources
                .filter(|(r, _)| r.pumpable(cm.rank))
                .map(|(r, density)| (r, self.extraction_rate(&r, cm.rank, density)))
                .collect(),
        }
    }

    pub fn extraction_rate(&self, resource: &Resource, oprank: u8, density: f64) -> f64 {
        let rank = ((oprank - resource.min_rank()) as f64) * (self.rank as f64);
        let difficulty = resource.extraction_difficulty().powf(EXRATE_DIFF_FACT);
        density * (rank / difficulty).powf(EXRATE_FACT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crew::CrewMember;
    use crate::galaxy::planet::Planet;
    use strum::IntoEnumIterator;

    fn rng() -> rand::rngs::ThreadRng {
        rand::rng()
    }

    fn solid_planet() -> Planet {
        let mut r = rng();
        loop {
            let p = Planet::random((0, 0, 0), &mut r);
            if p.resource_density(&Resource::Iron) > 0.0 {
                return p;
            }
        }
    }

    #[test]
    fn test_new_module_defaults() {
        let m = ShipModuleType::Miner.new_module();
        assert_eq!(m.modtype, ShipModuleType::Miner);
        assert_eq!(m.rank, 1);
        assert_eq!(m.totalcost, 0.0);
        assert!(m.operator.is_none());
    }

    #[test]
    fn test_get_price_buy_all_types() {
        for t in ShipModuleType::iter() {
            assert!(t.get_price_buy() > 0.0, "price <= 0 for {t:?}");
        }
    }

    #[test]
    fn test_price_next_rank_increases() {
        let mut m = ShipModuleType::Pump.new_module();
        let p1 = m.price_next_rank();
        m.rank = 5;
        let p5 = m.price_next_rank();
        assert!(p1 > 0.0);
        assert!(p5 > p1);
    }

    #[test]
    fn test_need_operator_only_when_unassigned() {
        let mut m = ShipModuleType::Miner.new_module();
        assert!(m.need(&CrewMemberType::Operator));
        assert!(!m.need(&CrewMemberType::Pilot));
        m.operator = Some(1);
        assert!(!m.need(&CrewMemberType::Operator));
    }

    #[test]
    fn test_can_extract_without_operator_is_empty() {
        let m = ShipModuleType::Miner.new_module();
        let crew = Crew::default();
        let planet = Planet::random((0, 0, 0), &mut rng());
        assert!(m.can_extract(&crew, &planet).is_empty());
    }

    #[test]
    fn test_extraction_rate_increases_with_module_rank() {
        let mut m = ShipModuleType::Miner.new_module();
        let r1 = m.extraction_rate(&Resource::Carbon, 8, 6.25);
        m.rank = 4;
        let r4 = m.extraction_rate(&Resource::Carbon, 8, 6.25);
        assert!(r1 > 0.0);
        assert!(r4 > r1);
    }

    #[test]
    fn test_extraction_rate_scales_with_density() {
        let m = ShipModuleType::Miner.new_module();
        let low = m.extraction_rate(&Resource::Iron, 8, 1.0);
        let high = m.extraction_rate(&Resource::Iron, 8, 6.25);
        assert!(high > low);
    }

    #[test]
    fn test_can_extract_miner_returns_mineable_resources() {
        let mut m = ShipModuleType::Miner.new_module();
        m.operator = Some(1);
        let mut crew = Crew::default();
        crew.0.insert(
            1,
            CrewMember {
                member_type: CrewMemberType::Operator,
                rank: 8,
            },
        );
        let planet = solid_planet();
        let extracted = m.can_extract(&crew, &planet);
        assert!(!extracted.is_empty());
        // A miner only yields mineable resources, each at a positive rate
        assert!(extracted
            .iter()
            .all(|(r, rate)| r.mineable(8) && *rate > 0.0));
    }

    #[test]
    fn test_can_extract_gas_sucker_on_solid_planet() {
        let mut m = ShipModuleType::GasSucker.new_module();
        m.operator = Some(1);
        let mut crew = Crew::default();
        crew.0.insert(
            1,
            CrewMember {
                member_type: CrewMemberType::Operator,
                rank: 8,
            },
        );
        let planet = solid_planet();
        // Solid planets also expose suckable gases (density rules)
        let extracted = m.can_extract(&crew, &planet);
        assert!(extracted.iter().all(|(r, _)| r.suckable(8)));
    }
}
