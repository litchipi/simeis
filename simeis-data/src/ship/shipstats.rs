use serde::{Deserialize, Serialize};

use crate::{
    config::{get_config, SimeisConfig},
    crew::{CrewMember, CrewMemberType},
};

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
pub struct ShipStats {
    pub speed: f64,
    pub fuel_consumption: f64,
    pub hull_usage_rate: f64,
}

impl ShipStats {
    pub fn new(rpow: u16, spow: u16, pilot: Option<&CrewMember>) -> ShipStats {
        let cfg = get_config();
        let hull_usage_rate = compute_hull_usage_ratio(cfg, spow as f64);
        let mut fuel_consumption = rpow as f64;
        let mut speed = 0.0;
        if let Some(pilot) = pilot {
            debug_assert!(matches!(pilot.member_type, CrewMemberType::Pilot));
            let totshare = (cfg.ship.pilot_fuel_share * 10) as f64;
            let num = (totshare - (pilot.rank as f64).min(totshare)).sqrt();
            fuel_consumption *= num / totshare;
            speed = (rpow as f64)
                * cfg.ship.reactor_speed_rate
                * (pilot.rank as f64).powf(cfg.ship.pilot_rank_speed_powf);
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

pub fn compute_hull_usage_ratio(cfg: &SimeisConfig, spow: f64) -> f64 {
    cfg.ship.hull_usage_rate / (1.0 + (1.0 + spow).log(cfg.ship.shield_rank_log))
}
