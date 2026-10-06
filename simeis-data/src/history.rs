use std::collections::VecDeque;

use serde::Serialize;
use serde_json::Value;

use crate::{
    crew::{CrewId, CrewMemberType},
    galaxy::station::StationId,
    industry::{IndustryUnitId, IndustryUnitType},
    player::{Player, PLAYER_ACTION_HISTORY_LEN},
    ship::{
        module::{ShipModuleId, ShipModuleType},
        resources::Resource,
        upgrade::{ShipExpandUpgrade, ShipUpgrade},
        ShipId,
    },
};

pub type PlayerHistory = VecDeque<PlayerHistoryEvent>;

#[derive(strum::IntoStaticStr, Serialize, Debug, Clone, Copy)]
#[strum(serialize_all = "snake_case")]
pub enum PlayerHistoryEvent {
    // Ship
    BuyShip {
        id: ShipId,
        station: StationId,
        price: f64,
    },
    BuyShipModule {
        ship: ShipId,
        id: ShipModuleId,
        modtype: ShipModuleType,
        price: f64,
        station: StationId,
    },
    UpgradeModule {
        ship: ShipId,
        station: StationId,
        id: ShipModuleId,
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
        id: CrewId,
        station: StationId,
    },
    FireCrew {
        id: CrewId,
    },
    AssignCrew {
        id: CrewId,
        ship: ShipId,
    },
    UnassignCrew {
        id: CrewId,
        ship: ShipId,
    },
    UpgradeCrew {
        id: CrewId,
        price: f64,
        newrank: u8,
        station: StationId,
    },

    // Industry
    BuyIndustryUnit {
        station: StationId,
        unit: IndustryUnitType,
        id: IndustryUnitId,
    },
    StartIndustry {
        id: IndustryUnitId,
    },
    StopIndustry {
        id: IndustryUnitId,
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
    for el in h.iter() {
        let Value::Object(mut obj) = serde_json::to_value(el).unwrap() else {
            unreachable!();
        };
        let key: &'static str = el.into();
        let Value::Object(mut data) = obj.remove(key).unwrap() else {
            unreachable!();
        };
        data.insert("action".to_string(), Value::String(key.to_string()));
        res.push(Value::Object(data));
    }
    res
}

impl Player {
    pub fn add_history(&mut self, event: PlayerHistoryEvent) {
        debug_assert!(self.history.len() <= PLAYER_ACTION_HISTORY_LEN);
        self.history.push_back(event);
        while self.history.len() > PLAYER_ACTION_HISTORY_LEN {
            self.history.pop_front();
        }
    }
}
