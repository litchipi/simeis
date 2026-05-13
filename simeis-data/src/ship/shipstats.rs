use serde::{Deserialize, Serialize};

use crate::crew::{CrewMember, CrewMemberType};

const HULL_USAGE_BASE: f64 = 5.0 / 100.0;
const REACTOR_SPEED_PER_POWER: f64 = 50.0;
const PILOT_FUEL_SHARE: u8 = 5; // Rank 10 = 4/5 fuel consumption

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
pub struct ShipStats {
    pub speed: f64,
    pub fuel_consumption: f64,
    pub hull_usage_rate: f64,
}

impl ShipStats {
    pub fn new(rpow: u16, spow: u16, pilot: Option<&CrewMember>) -> ShipStats {
        let hull_usage_rate = HULL_USAGE_BASE / (1.0 + (1.0 + spow as f64).log(3.5));
        let mut fuel_consumption = rpow as f64;
        let mut speed = 0.0;
        if let Some(pilot) = pilot {
            debug_assert!(matches!(pilot.member_type, CrewMemberType::Pilot));
            let totshare = (PILOT_FUEL_SHARE * 10) as f64;
            let num = (totshare - (pilot.rank as f64).min(totshare)).sqrt();
            fuel_consumption *= num / totshare;
            speed = (rpow as f64) * REACTOR_SPEED_PER_POWER * (pilot.rank as f64);
        }
        #[cfg(feature = "extraspeed")]
        {
            speed *= 1000.0;
            fuel_consumption *= 1000.0;
        }
        ShipStats {
            speed,
            hull_usage_rate,
            fuel_consumption,
        }
    }
}
