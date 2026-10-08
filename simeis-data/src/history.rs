use std::{collections::VecDeque, time::Instant};

use serde::Serialize;
use serde_json::Value;

use crate::crew::{CrewId, CrewMemberType};
use crate::galaxy::station::StationId;
use crate::industry::{IndustryUnitId, IndustryUnitType};
use crate::player::{Player, PLAYER_ACTION_HISTORY_LEN};

use crate::ship::module::{ShipModuleId, ShipModuleType};
use crate::ship::resources::Resource;
use crate::ship::upgrade::{ShipExpandUpgrade, ShipUpgrade};
use crate::ship::ShipId;

pub type PlayerHistory = VecDeque<(Instant, PlayerHistoryEvent)>;

#[derive(strum::IntoStaticStr, Debug, Clone, Copy, Serialize)]
#[strum(serialize_all = "snake_case")]
pub enum PlayerHistoryEvent {
    // Ship
    BuyShip {
        ship: ShipId,
        station: StationId,
        price: f64,
    },
    BuyShipModule {
        ship: ShipId,
        module: ShipModuleId,
        modtype: ShipModuleType,
        price: f64,
        station: StationId,
    },
    UpgradeModule {
        ship: ShipId,
        station: StationId,
        module: ShipModuleId,
        newrank: u8,
        price: f64,
    },
    UpgradeShip {
        ship: ShipId,
        upgrade: ShipUpgrade,
        price: f64,
        station: StationId,
    },
    ExpandShip {
        ship: ShipId,
        expansion: ShipExpandUpgrade,
        price: f64,
        station: StationId,
    },

    // Station
    BuyStationCargo {
        station: StationId,
        amnt: usize,
        total_price: f64,
    },

    // Crew
    HireCrew {
        crewtype: CrewMemberType,
        crew: CrewId,
        station: StationId,
    },
    FireCrew {
        crew: CrewId,
        station: StationId,
    },
    AssignCrew {
        crew: CrewId,
        ship: ShipId,
    },
    UnassignCrew {
        crew: CrewId,
        station: StationId,
    },
    UpgradeCrew {
        crew: CrewId,
        price: f64,
        newrank: u8,
        station: StationId,
    },

    // Industry
    BuyIndustryUnit {
        station: StationId,
        unittype: IndustryUnitType,
        unit: IndustryUnitId,
    },
    StartIndustry {
        unit: IndustryUnitId,
    },
    StopIndustry {
        unit: IndustryUnitId,
    },

    // Market
    BuyResource {
        station: StationId,
        resource: Resource,
        price: f64,
        amnt: f64,
        fee: f64,
    },
    SellResource {
        station: StationId,
        resource: Resource,
        price: f64,
        amnt: f64,
        fee: f64,
    },
}

#[inline]
pub fn export_history(h: &PlayerHistory) -> Vec<Value> {
    let mut res = Vec::with_capacity(h.len());
    for (t, el) in h.iter().rev() {
        let Value::Object(mut obj) = serde_json::to_value(el).unwrap() else {
            unreachable!();
        };
        debug_assert_eq!(obj.len(), 1);
        let mapkey = obj.keys().next().unwrap().clone();
        let Value::Object(mut data) = obj.remove(&mapkey).unwrap() else {
            unreachable!();
        };
        let key: &'static str = el.into();
        data.insert("action".to_string(), Value::String(key.to_string()));
        data.insert(
            "elapsed".to_string(),
            Value::Number(serde_json::Number::from_f64(t.elapsed().as_secs_f64()).unwrap()),
        );
        res.push(Value::Object(data));
    }
    res
}

impl Player {
    pub fn add_history(&mut self, event: PlayerHistoryEvent) {
        debug_assert!(self.history.len() <= PLAYER_ACTION_HISTORY_LEN);
        self.history.push_back((Instant::now(), event));
        while self.history.len() > PLAYER_ACTION_HISTORY_LEN {
            self.history.pop_front();
        }
    }
}
