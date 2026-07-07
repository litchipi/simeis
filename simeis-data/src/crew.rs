use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use strum::{EnumString, IntoStaticStr};

const WAGE_INC_RANK_POWF: f64 = 0.85;
const RANK_PRICE_WAGE_MULT: f64 = 1900.0;

pub type CrewId = u32;

#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct Crew(pub BTreeMap<CrewId, CrewMember>);
impl Crew {
    pub fn sum_wages(&self) -> f64 {
        self.0.values().map(|crew| crew.wage()).sum::<f64>()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrewMember {
    pub member_type: CrewMemberType,
    pub rank: u8,
}

impl From<CrewMemberType> for CrewMember {
    fn from(member_type: CrewMemberType) -> Self {
        CrewMember {
            member_type,
            rank: 1,
        }
    }
}

impl CrewMember {
    pub fn wage(&self) -> f64 {
        let op = 0.75;
        let base = match self.member_type {
            CrewMemberType::Operator => op,
            CrewMemberType::Pilot => op * 8.0,
            CrewMemberType::Trader => op * 5.0,
            CrewMemberType::Soldier => op * 2.5,
        };
        base * (self.rank as f64).powf(WAGE_INC_RANK_POWF)
    }

    #[inline]
    pub fn price_next_rank(&self) -> f64 {
        self.wage().powf(1.75) * RANK_PRICE_WAGE_MULT
    }
}

#[allow(dead_code)]
#[derive(EnumString, IntoStaticStr, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[strum(ascii_case_insensitive)]
pub enum CrewMemberType {
    Pilot,
    Operator,
    Trader,
    Soldier,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_crew_member_type_default_rank_1() {
        let cm = CrewMember::from(CrewMemberType::Pilot);
        assert_eq!(cm.rank, 1);
        assert_eq!(cm.member_type, CrewMemberType::Pilot);
    }

    #[test]
    fn test_wage_ordering_by_type_at_rank_1() {
        // At rank 1, the rank multiplier is 1, so wages follow the base factors:
        // Pilot (8x) > Trader (5x) > Soldier (2.5x) > Operator (1x)
        let op = CrewMember::from(CrewMemberType::Operator).wage();
        let pilot = CrewMember::from(CrewMemberType::Pilot).wage();
        let trader = CrewMember::from(CrewMemberType::Trader).wage();
        let soldier = CrewMember::from(CrewMemberType::Soldier).wage();
        assert!(pilot > trader);
        assert!(trader > soldier);
        assert!(soldier > op);
        assert!(op > 0.0);
    }

    #[test]
    fn test_wage_increases_with_rank() {
        let mut prev = 0.0;
        for rank in 1..=10u8 {
            let cm = CrewMember {
                member_type: CrewMemberType::Operator,
                rank,
            };
            let w = cm.wage();
            assert!(w > prev, "wage not increasing at rank {rank}");
            prev = w;
        }
    }

    #[test]
    fn test_price_next_rank_grows_with_wage() {
        let low = CrewMember {
            member_type: CrewMemberType::Operator,
            rank: 1,
        };
        let high = CrewMember {
            member_type: CrewMemberType::Pilot,
            rank: 5,
        };
        assert!(low.price_next_rank() > 0.0);
        assert!(high.price_next_rank() > low.price_next_rank());
    }

    #[test]
    fn test_sum_wages_empty_is_zero() {
        let crew = Crew::default();
        assert_eq!(crew.sum_wages(), 0.0);
    }

    #[test]
    fn test_sum_wages_is_additive() {
        let mut crew = Crew::default();
        crew.0.insert(1, CrewMember::from(CrewMemberType::Pilot));
        crew.0.insert(2, CrewMember::from(CrewMemberType::Operator));
        let expected = CrewMember::from(CrewMemberType::Pilot).wage()
            + CrewMember::from(CrewMemberType::Operator).wage();
        assert!((crew.sum_wages() - expected).abs() < 1e-9);
    }
}
