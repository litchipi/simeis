use std::collections::BTreeMap;

use rand::RngExt;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumString, IntoStaticStr};

use crate::{
    crew::{CrewId, CrewMember, CrewMemberType},
    ship::resources::Resource,
};

pub type IndustryUnitId = u32;

const UNIT_UPG_POWF_DIV: f64 = 75.0;

const SBASE_REQ: f64 = 1.5;
const ABASE_REQ: f64 = 7.5;

// Because all resources of the same level have the same base price
// The resource cost (in credits) should be the same whatever the unit is
// As long as it's the same class (simple / advanced)
pub const fn get_simple_industry_resources_cost() -> f64 {
    (SBASE_REQ * Resource::Hydrogen.base_price())
        + (SBASE_REQ * 0.2 * Resource::Oxygen.base_price())
        + (SBASE_REQ * 1.25 * Resource::Carbon.base_price())
        + (SBASE_REQ * 0.4 * Resource::Water.base_price())
}

pub const fn get_advanced_industry_resources_cost() -> f64 {
    (ABASE_REQ * Resource::Carbon.base_price())
        + (ABASE_REQ * 0.4 * Resource::Oil.base_price())
        + (ABASE_REQ * 0.2 * Resource::Helium.base_price())
}

pub const fn get_sbase_produce_base() -> f64 {
    let scost = get_simple_industry_resources_cost();
    scost / (1.05 * Resource::Fuel.base_price())
}

pub const fn get_abase_produce_base() -> f64 {
    let acost = get_advanced_industry_resources_cost();
    acost / (1.75 * Resource::Fuel.base_price())
}

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
pub enum IndustryUnitType {
    SimpleFuelRefinery,
    AdvancedFuelRefinery,

    SimpleHullFoundry,
    AdvancedHullFoundry,
}

impl IndustryUnitType {
    pub fn new_unit(self) -> IndustryUnit {
        let unitid = rand::rng().random();
        IndustryUnit {
            id: unitid,
            operator: None,
            unittype: self,
            rank: 1,
            started: false,
            resources_required: vec![],
            resources_created: vec![],
        }
    }

    #[inline]
    pub fn get_price_buy(&self) -> f64 {
        match self {
            IndustryUnitType::SimpleHullFoundry | IndustryUnitType::SimpleFuelRefinery => 8000.0,
            IndustryUnitType::AdvancedHullFoundry | IndustryUnitType::AdvancedFuelRefinery => {
                18000.0
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IndustryUnit {
    pub id: IndustryUnitId,
    pub unittype: IndustryUnitType,
    pub rank: u8,
    pub started: bool,

    pub operator: Option<CrewId>,
    resources_required: Vec<(Resource, f64)>,
    resources_created: Vec<(Resource, f64)>,
}

impl IndustryUnit {
    #[inline]
    pub fn price_next_rank(&self) -> f64 {
        let num = UNIT_UPG_POWF_DIV - 1.0 + (self.rank as f64);
        self.unittype.get_price_buy().powf(num / UNIT_UPG_POWF_DIV)
    }

    #[inline]
    pub fn need_crew_member(&self, ctype: &CrewMemberType) -> bool {
        ctype == &CrewMemberType::Operator && self.operator.is_none()
    }

    #[inline]
    pub fn assign_operator(&mut self, opid: CrewId, op: &CrewMember) {
        self.operator = Some(opid);
        self.new_op_rank(op.rank);
    }

    #[inline]
    pub fn new_op_rank(&mut self, rank: u8) {
        self.resources_required = self.input(rank);
        self.resources_created = self.output(rank);
    }

    #[inline]
    pub fn input(&self, oprank: u8) -> Vec<(Resource, f64)> {
        debug_assert_ne!(oprank, 0);
        let div = 1.0 / (std::f64::consts::E + (oprank as f64) - 1.0).ln();
        match self.unittype {
            IndustryUnitType::SimpleFuelRefinery => {
                let sbase = SBASE_REQ * (self.rank as f64);
                vec![
                    (Resource::Hydrogen, sbase),      // Gas 1
                    (Resource::Oxygen, sbase * 0.2),  // Gas 2
                    (Resource::Carbon, sbase * 1.25), // Solid 1
                    (Resource::Water, sbase * 0.4),   // Liquid 1
                ]
            }
            IndustryUnitType::SimpleHullFoundry => {
                let sbase = SBASE_REQ * (self.rank as f64);
                vec![
                    (Resource::Carbon, sbase),          // Solid 1
                    (Resource::Iron, sbase * 0.2),      // Solid 2
                    (Resource::Hydrogen, sbase * 1.25), // Gas 1
                    (Resource::Water, 0.5 * 0.4),       // Liquid 1
                ]
            }
            IndustryUnitType::AdvancedFuelRefinery => {
                let abase = ABASE_REQ * (self.rank as f64);
                vec![
                    (Resource::Carbon, abase),       // Solid 1
                    (Resource::Oil, abase * 0.4),    // Liquid 3
                    (Resource::Helium, abase * 0.2), // Gas 3
                ]
            }
            IndustryUnitType::AdvancedHullFoundry => {
                let abase = ABASE_REQ * (self.rank as f64);
                vec![
                    (Resource::Hydrogen, abase),     // Gas 1
                    (Resource::Copper, abase * 0.4), // Solid 3
                    (Resource::Oil, abase * 0.2),    // Liquid 3
                ]
            }
        }
        .into_iter()
        .map(|(res, amnt)| {
            let amnt: f64 = amnt;
            let new_amnt = amnt.powf(div);
            (res, new_amnt)
        })
        .collect()
    }

    #[inline]
    pub fn output(&self, oprank: u8) -> Vec<(Resource, f64)> {
        debug_assert_ne!(oprank, 0);
        let pown = (oprank as f64).ln();

        match self.unittype {
            IndustryUnitType::SimpleFuelRefinery => vec![(
                Resource::Fuel,
                get_sbase_produce_base() * (self.rank as f64),
            )],
            IndustryUnitType::SimpleHullFoundry => vec![(
                Resource::Hull,
                get_sbase_produce_base() * (self.rank as f64),
            )],
            IndustryUnitType::AdvancedFuelRefinery => vec![(
                Resource::Fuel,
                get_abase_produce_base() * (self.rank as f64),
            )],
            IndustryUnitType::AdvancedHullFoundry => vec![(
                Resource::Hull,
                get_abase_produce_base() * (self.rank as f64),
            )],
        }
        .into_iter()
        .map(|(res, amnt)| {
            let amnt: f64 = amnt;
            (res, amnt.powf(pown))
        })
        .collect()
    }

    pub fn can_work(&self, tdelta: &f64, resources: &BTreeMap<Resource, f64>) -> Option<f64> {
        if !self.started {
            return None;
        }
        self.operator?;
        let mut max_ratio: f64 = 0.0;
        for (res, amnt) in self.resources_required.iter() {
            if let Some(incargo) = resources.get(res) {
                let max = amnt * tdelta;
                if incargo >= &max {
                    max_ratio = 1.0;
                } else {
                    let ratio = incargo / max;
                    max_ratio = max_ratio.max(ratio);
                }
            } else {
                return None;
            }
        }
        Some(max_ratio)
    }

    pub fn work(&self, tdelta: f64, resources: &mut BTreeMap<Resource, f64>) {
        debug_assert!(self.started);
        debug_assert!(self.operator.is_some());
        for (res, amnt) in self.resources_required.iter() {
            let n = resources.get_mut(res).unwrap();
            *n -= amnt * tdelta;
            log::warn!("Used {} of {res:?}, got {n} left", amnt * tdelta);
        }

        for (res, amnt) in self.resources_created.iter() {
            if !resources.contains_key(res) {
                resources.insert(*res, 0.0);
            }

            let n = resources.get_mut(res).unwrap();
            *n += amnt * tdelta;
            log::warn!("Created {} of {res:?}, got {n} now", amnt * tdelta);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_resource_cost_constants_positive() {
        assert!(get_simple_industry_resources_cost() > 0.0);
        assert!(get_advanced_industry_resources_cost() > 0.0);
        assert!(get_sbase_produce_base() > 0.0);
        assert!(get_abase_produce_base() > 0.0);
        // Advanced units consume more expensive resources than simple ones
        assert!(get_advanced_industry_resources_cost() > get_simple_industry_resources_cost());
    }

    #[test]
    fn test_get_price_buy_tiers() {
        assert_eq!(IndustryUnitType::SimpleFuelRefinery.get_price_buy(), 8000.0);
        assert_eq!(IndustryUnitType::SimpleHullFoundry.get_price_buy(), 8000.0);
        assert_eq!(
            IndustryUnitType::AdvancedFuelRefinery.get_price_buy(),
            18000.0
        );
        assert_eq!(
            IndustryUnitType::AdvancedHullFoundry.get_price_buy(),
            18000.0
        );
    }

    #[test]
    fn test_new_unit_defaults() {
        for t in IndustryUnitType::iter() {
            let u = t.clone().new_unit();
            assert_eq!(u.unittype, t);
            assert_eq!(u.rank, 1);
            assert!(!u.started);
            assert!(u.operator.is_none());
        }
    }

    #[test]
    fn test_price_next_rank_increases() {
        let mut u = IndustryUnitType::SimpleFuelRefinery.new_unit();
        let p1 = u.price_next_rank();
        u.rank = 6;
        let p6 = u.price_next_rank();
        assert!(p1 > 0.0);
        assert!(p6 > p1);
    }

    #[test]
    fn test_need_crew_member_only_idle_operator() {
        let mut u = IndustryUnitType::SimpleHullFoundry.new_unit();
        assert!(u.need_crew_member(&CrewMemberType::Operator));
        assert!(!u.need_crew_member(&CrewMemberType::Pilot));
        u.operator = Some(1);
        assert!(!u.need_crew_member(&CrewMemberType::Operator));
    }

    #[test]
    fn test_input_output_non_empty_and_positive() {
        for t in IndustryUnitType::iter() {
            let u = t.new_unit();
            let inputs = u.input(3);
            let outputs = u.output(3);
            assert!(!inputs.is_empty());
            assert!(!outputs.is_empty());
            for (_, amnt) in inputs.iter().chain(outputs.iter()) {
                assert!(*amnt > 0.0);
            }
        }
    }

    #[test]
    fn test_refineries_produce_fuel_foundries_produce_hull() {
        let fuel = IndustryUnitType::SimpleFuelRefinery.new_unit().output(2);
        assert!(fuel.iter().all(|(r, _)| *r == Resource::Fuel));
        let hull = IndustryUnitType::SimpleHullFoundry.new_unit().output(2);
        assert!(hull.iter().all(|(r, _)| *r == Resource::Hull));
    }

    #[test]
    fn test_assign_operator_sets_recipes() {
        let mut u = IndustryUnitType::SimpleFuelRefinery.new_unit();
        let op = CrewMember {
            member_type: CrewMemberType::Operator,
            rank: 2,
        };
        u.assign_operator(7, &op);
        assert_eq!(u.operator, Some(7));
        assert!(!u.resources_required.is_empty());
        assert!(!u.resources_created.is_empty());
    }

    #[test]
    fn test_can_work_requires_started_and_operator() {
        let mut u = IndustryUnitType::SimpleFuelRefinery.new_unit();
        let resources = BTreeMap::new();
        // Not started yet
        assert!(u.can_work(&1.0, &resources).is_none());
        u.started = true;
        // Started but no operator
        assert!(u.can_work(&1.0, &resources).is_none());
    }

    #[test]
    fn test_can_work_returns_none_when_missing_resource() {
        let mut u = IndustryUnitType::SimpleFuelRefinery.new_unit();
        let op = CrewMember {
            member_type: CrewMemberType::Operator,
            rank: 1,
        };
        u.assign_operator(1, &op);
        u.started = true;
        // Empty cargo: required resources missing -> None
        let resources = BTreeMap::new();
        assert!(u.can_work(&1.0, &resources).is_none());
    }

    #[test]
    fn test_work_consumes_inputs_and_creates_outputs() {
        let mut u = IndustryUnitType::SimpleFuelRefinery.new_unit();
        let op = CrewMember {
            member_type: CrewMemberType::Operator,
            rank: 1,
        };
        u.assign_operator(1, &op);
        u.started = true;

        let mut resources = BTreeMap::new();
        for (res, _) in u.input(1) {
            resources.insert(res, 1_000.0);
        }
        let ratio = u.can_work(&1.0, &resources).unwrap();
        assert_eq!(ratio, 1.0);

        let hydrogen_before = resources[&Resource::Hydrogen];
        u.work(1.0, &mut resources);
        assert!(resources[&Resource::Hydrogen] < hydrogen_before);
        assert!(resources.get(&Resource::Fuel).copied().unwrap_or(0.0) > 0.0);
    }
}
