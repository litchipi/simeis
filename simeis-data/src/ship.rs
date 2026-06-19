use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::crew::{Crew, CrewId, CrewMemberType};
use crate::errors::Errcode;
use crate::galaxy::planet::Planet;
use crate::galaxy::station::Station;
use crate::galaxy::{translation, SpaceCoord};
use crate::player::PlayerId;

pub mod cargo;
pub mod module;
pub mod navigation;
pub mod resources;
pub mod shipstats;
pub mod upgrade;

use cargo::ShipCargo;
use module::{ShipModule, ShipModuleId};
use navigation::{FlightData, Travel, TravelCost};
use resources::{ExtractionInfo, Resource};
use shipstats::ShipStats;

const PILOT_FUEL_SHARE: u8 = 5; // Rank 10 = 4/5 fuel consumption
const HULL_USAGE_BASE: f64 = 5.0 / 100.0;

const FUEL_TANK_CAP_PRICE: f64 = 30.0;
const CARGO_CAP_PRICE: f64 = 20.0;
const HULL_RESIS_PRICE: f64 = 9.0;
const REACTOR_POWER_PRICE: f64 = 4000.0;
const SHIELD_PRICE: f64 = 2500.0;

const REACTOR_SPEED_PER_POWER: f64 = 50.0;

pub type ShipId = u64;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub enum ShipState {
    #[default]
    Idle,
    InFlight(FlightData),
    Extracting(ExtractionInfo),
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct Ship {
    pub id: ShipId,
    pub reactor_power: u16,
    pub fuel_tank_capacity: f64,
    pub hull_resistance: f64,
    pub modules: BTreeMap<ShipModuleId, ShipModule>,
    pub shield_power: u16,

    #[serde(default)]
    pub owner: PlayerId,
    #[serde(default)]
    pub position: SpaceCoord,
    #[serde(default)]
    pub crew: Crew,
    #[serde(default)]
    pub cargo: ShipCargo,
    #[serde(default)]
    pub fuel_tank: f64,
    #[serde(default)]
    pub hull_decay: f64,
    #[serde(default)]
    pub pilot: Option<CrewId>,
    #[serde(default)]
    pub state: ShipState,
    #[serde(default)]
    pub stats: shipstats::ShipStats,
}

impl Ship {
    pub fn init_shipyard(position: SpaceCoord) -> Vec<Ship> {
        let mut rng = rand::rng();
        vec![
            Ship::light(rng.random(), position),
            Ship::medium(rng.random(), position),
            Ship::heavy(rng.random(), position),
        ]
    }

    pub fn random(position: SpaceCoord) -> Ship {
        let mut rng = rand::rng();
        let cargo_cap = rng.random_range(10.0..1000.0) as f64;
        Ship {
            id: rng.random(),
            position,
            reactor_power: rng.random_range(1..10),
            fuel_tank_capacity: rng.random_range(1..10000) as f64,
            cargo: ShipCargo::with_capacity(cargo_cap),
            hull_resistance: rng.random_range(1000..50000) as f64,
            ..Default::default()
        }
    }

    fn light(id: ShipId, position: SpaceCoord) -> Ship {
        Ship {
            id,
            position,
            reactor_power: 1,
            fuel_tank_capacity: 1000.0,
            cargo: ShipCargo::with_capacity(200.0),
            hull_resistance: 3000.0,
            shield_power: 0,
            ..Default::default()
        }
    }

    fn medium(id: ShipId, position: SpaceCoord) -> Ship {
        Ship {
            id,
            position,
            reactor_power: 3,
            fuel_tank_capacity: 2000.0,
            cargo: ShipCargo::with_capacity(400.0),
            hull_resistance: 6000.0,
            shield_power: 1,
            ..Default::default()
        }
    }

    fn heavy(id: ShipId, position: SpaceCoord) -> Ship {
        Ship {
            id,
            position,
            reactor_power: 10,
            fuel_tank_capacity: 4000.0,
            cargo: ShipCargo::with_capacity(1200.0),
            hull_resistance: 20000.0,
            shield_power: 3,
            ..Default::default()
        }
    }

    //     Change it every X minutes
    //     Used by traders to seek nice ships to buy

    // Public data of this ship to display on the marketplace
    pub fn market_data(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "price": self.compute_price(),
            "modules": self.modules,
            "reactor_power": self.reactor_power,
            "cargo_capacity": self.cargo.capacity,
            "fuel_tank_capacity": self.fuel_tank_capacity,
            "hull_resistance": self.hull_resistance,
        })
    }

    pub fn compute_price(&self) -> f64 {
        let mut price = 0.0;
        price += (self.reactor_power as f64) * REACTOR_POWER_PRICE;
        price += self.fuel_tank_capacity * FUEL_TANK_CAP_PRICE;
        price += self.cargo.capacity * CARGO_CAP_PRICE;
        price += self.hull_resistance * HULL_RESIS_PRICE;
        price += self.modules.values().map(|m| m.totalcost).sum::<f64>();
        price
    }

    // Updates the performances of the ship based on the crew onboard
    pub fn update_perf_stats(&mut self) {
        self.stats = ShipStats::default();
        self.stats.hull_usage_rate =
            HULL_USAGE_BASE / (1.0 + (1.0 + self.shield_power as f64).log(3.5));
        self.stats.fuel_consumption = self.reactor_power as f64;

        if let Some(ref pilot) = self.pilot {
            let pilot = self.crew.0.get(pilot).unwrap();
            debug_assert!(matches!(pilot.member_type, CrewMemberType::Pilot));
            let totshare = (PILOT_FUEL_SHARE * 10) as f64;
            let num = (totshare - (pilot.rank as f64).min(totshare)).sqrt();
            self.stats.fuel_consumption *= num / totshare;
            self.stats.speed =
                (self.reactor_power as f64) * REACTOR_SPEED_PER_POWER * (pilot.rank as f64);
        } else {
            self.stats.speed = 0.0;
        };
        #[cfg(feature = "extraspeed")]
        {
            self.stats.speed *= 1000.0;
            self.stats.fuel_consumption *= 1000.0;
        }
        self.stats.speed *= 1.0 - self.cargo.slowing_ratio();
    }

    pub fn compute_travel_costs(&self, destination: SpaceCoord) -> Result<TravelCost, Errcode> {
        let travel = Travel::new(destination);
        let cost = travel.compute_costs(self)?;
        Ok(cost)
    }

    pub fn set_travel(&mut self, destination: SpaceCoord) -> Result<TravelCost, Errcode> {
        let ShipState::Idle = self.state else {
            return Err(Errcode::ShipNotIdle);
        };
        let travel = Travel::new(destination);
        let cost = travel.compute_costs(self)?;
        if !cost.have_enough(self) {
            return Err(Errcode::CannotPerformTravel);
        }
        log::debug!("Starting flight on ship {}", self.id);
        self.state = ShipState::InFlight(FlightData::new(self.position, &cost, &travel));
        Ok(cost)
    }

    pub fn update_flight(&mut self, mut tdelta: f64) -> bool {
        let ShipState::InFlight(ref mut data) = self.state else {
            unreachable!();
        };

        let mut finished = false;
        let mut dist_delta = self.stats.speed * tdelta;
        data.dist_done += dist_delta;
        if data.dist_done >= data.dist_tot {
            finished = true;
            let doverflow = data.dist_done - data.dist_tot;
            data.dist_done -= doverflow;
            dist_delta -= doverflow;

            let toverflow = doverflow / self.stats.speed;
            tdelta -= toverflow;
            debug_assert!(((tdelta * self.stats.speed) - dist_delta).abs() < 1e-7);
        }

        self.position = translation(data.start, data.direction, data.dist_done);

        self.fuel_tank -= self.stats.fuel_consumption * tdelta;
        if self.fuel_tank <= 0.0 {
            self.fuel_tank = 0.0;
            log::debug!("Ship {} has an empty fuel tank", self.id);
            return true;
        }

        self.hull_decay += self.stats.hull_usage_rate * dist_delta;
        if self.hull_decay >= self.hull_resistance {
            log::debug!("Ship {} worn out all its hull", self.id);
            return true;
        }

        if finished {
            debug_assert_eq!(self.position, data.destination);
        }
        finished
    }

    pub fn stop_navigation(&mut self) -> Result<SpaceCoord, Errcode> {
        log::debug!("Stopping flight on ship {}", self.id);
        self.state = ShipState::Idle;
        Ok(self.position)
    }

    pub async fn start_extraction(&mut self, planet: &Planet) -> Result<ExtractionInfo, Errcode> {
        let ShipState::Idle = self.state else {
            return Err(Errcode::ShipNotIdle);
        };
        log::debug!(
            "Ship {} started extraction on planet {:?}",
            self.id,
            planet.position
        );

        let extraction = ExtractionInfo::create(self, planet);
        if !extraction.mining_rate.is_empty() {
            self.state = ShipState::Extracting(extraction.clone());
        } else {
            return Err(Errcode::CannotExtractWithoutModule);
        }
        log::debug!("Extraction of resources: {extraction:?}");
        Ok(extraction)
    }

    pub fn stop_extraction(&mut self) -> Result<(), Errcode> {
        let ShipState::Extracting(_) = self.state else {
            return Err(Errcode::ShipNotExtracting);
        };
        log::debug!("Ship {} stopped extraction", self.id);
        self.state = ShipState::Idle;
        Ok(())
    }

    pub fn update_extract(&mut self, tdelta: f64) -> bool {
        let ShipState::Extracting(ref rates) = self.state else {
            unreachable!();
        };
        rates.update_cargo(&mut self.cargo, tdelta)
    }

    pub async fn unload_all(
        &mut self,
        station: &Station,
    ) -> Result<BTreeMap<Resource, f64>, Errcode> {
        let all_resources = self.cargo.resources.clone();
        let mut unloaded = BTreeMap::new();
        for (res, amnt) in all_resources {
            let got = self.unload_cargo(&res, amnt, station).await?;
            unloaded.insert(res, got);
            if got < amnt {
                return Ok(unloaded);
            }
        }
        Ok(unloaded)
    }

    pub async fn unload_cargo(
        &mut self,
        resource: &Resource,
        amnt: f64,
        station: &Station,
    ) -> Result<f64, Errcode> {
        let unloaded = self.cargo.unload(resource, amnt);
        if unloaded == 0.0 {
            return Ok(0.0);
        }

        let added = station.add_resource(&self.owner, resource, unloaded).await;
        if added < unloaded {
            self.cargo.add_resource(resource, unloaded - added);
            Ok(added)
        } else {
            Ok(unloaded)
        }
    }
}

#[test]
fn test_ship_flight() {
    crate::tests::create_property_based_test(100000, &[], |rng| {
        let (x, y, z) = (rng.random(), rng.random(), rng.random());
        let mut ship = Ship::random((x, y, z));
        ship.fuel_tank = ship.fuel_tank_capacity;

        let pilot_id = rng.random();
        ship.crew.0.insert(
            pilot_id,
            crate::crew::CrewMember::from(CrewMemberType::Pilot),
        );
        ship.pilot = Some(pilot_id);
        ship.update_perf_stats();

        let add = rng.random_range(1..100);
        let dest = (
            x.saturating_add(add),
            y.saturating_add(add),
            z.saturating_add(add),
        );
        let res = ship.set_travel(dest);
        let init_state = ship.clone();
        if let Ok(costs) = res {
            assert!(costs.duration > 0.0);
            ship.update_flight(costs.duration / 2.0);
            let ShipState::InFlight(flight) = ship.state else {
                println!("Ship not in flight: {:?}", ship.state);
                unreachable!();
            };
            assert_eq!(flight.start, (x, y, z));
            assert_eq!(flight.destination, dest);
            assert!(flight.dist_done > 0.0);
            assert_ne!(flight.dist_done, flight.dist_tot);
            assert!(init_state.fuel_tank > ship.fuel_tank);
            assert!(ship.fuel_tank < ship.fuel_tank_capacity);
            assert!(ship.hull_decay > 0.0);
            assert_eq!(init_state.cargo.usage, ship.cargo.usage);
        } else {
            let travel = Travel::new(dest);
            let costs = travel.compute_costs(&ship).unwrap();
            assert!(
                (costs.fuel_consumption > ship.fuel_tank)
                    || (costs.hull_usage > ship.hull_resistance)
            );
        }
    });
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;
    use crate::crew::CrewMember;
    use crate::galaxy::planet::Planet;
    use crate::galaxy::station::Station;
    use crate::ship::module::ShipModuleType;
    use crate::tests::block_on;

    fn solid_planet() -> Planet {
        let mut r = rand::rng();
        loop {
            let p = Planet::random((0, 0, 0), &mut r);
            if p.resource_density(&Resource::Iron) > 0.0 {
                return p;
            }
        }
    }

    fn mining_ship() -> Ship {
        let mut ship = Ship::default();
        ship.cargo = ShipCargo::with_capacity(1000.0);
        let mut module = ShipModuleType::Miner.new_module();
        module.operator = Some(1);
        ship.modules.insert(1, module);
        ship.crew.0.insert(
            1,
            CrewMember {
                member_type: CrewMemberType::Operator,
                rank: 8,
            },
        );
        ship
    }

    #[test]
    fn test_init_shipyard_has_three_tiers() {
        let yard = Ship::init_shipyard((0, 0, 0));
        assert_eq!(yard.len(), 3);
        // Tiers are ordered light < medium < heavy in capability
        assert!(yard[0].cargo.capacity < yard[1].cargo.capacity);
        assert!(yard[1].cargo.capacity < yard[2].cargo.capacity);
        assert!(yard[0].reactor_power < yard[2].reactor_power);
    }

    #[test]
    fn test_compute_price_positive_and_monotonic() {
        let yard = Ship::init_shipyard((0, 0, 0));
        for ship in &yard {
            assert!(ship.compute_price() > 0.0);
        }
        // A heavier ship is worth more
        assert!(yard[2].compute_price() > yard[0].compute_price());
    }

    #[test]
    fn test_compute_price_increases_with_modules() {
        let mut ship = Ship::default();
        ship.reactor_power = 1;
        let base = ship.compute_price();
        let mut module = ShipModuleType::Miner.new_module();
        module.totalcost = 1234.0;
        ship.modules.insert(1, module);
        assert!((ship.compute_price() - (base + 1234.0)).abs() < 1e-6);
    }

    #[test]
    fn test_update_perf_stats_without_pilot_zero_speed() {
        let mut ship = Ship::default();
        ship.reactor_power = 5;
        ship.update_perf_stats();
        assert_eq!(ship.stats.speed, 0.0);
        assert_eq!(ship.stats.fuel_consumption, 5.0);
        assert!(ship.stats.hull_usage_rate > 0.0);
    }

    #[test]
    fn test_update_perf_stats_with_pilot_has_speed() {
        let mut ship = Ship::default();
        ship.reactor_power = 5;
        ship.crew
            .0
            .insert(1, CrewMember::from(CrewMemberType::Pilot));
        ship.pilot = Some(1);
        ship.update_perf_stats();
        assert!(ship.stats.speed > 0.0);
    }

    #[test]
    fn test_higher_pilot_rank_gives_more_speed() {
        let make = |rank: u8| {
            let mut ship = Ship::default();
            ship.reactor_power = 5;
            ship.crew.0.insert(
                1,
                CrewMember {
                    member_type: CrewMemberType::Pilot,
                    rank,
                },
            );
            ship.pilot = Some(1);
            ship.update_perf_stats();
            ship.stats.speed
        };
        assert!(make(3) > make(1));
    }

    #[test]
    fn test_shield_reduces_hull_usage_rate() {
        let mut no_shield = Ship::default();
        no_shield.shield_power = 0;
        no_shield.update_perf_stats();

        let mut shielded = Ship::default();
        shielded.shield_power = 3;
        shielded.update_perf_stats();

        assert!(shielded.stats.hull_usage_rate < no_shield.stats.hull_usage_rate);
    }

    #[test]
    fn test_stop_navigation_sets_idle() {
        let mut ship = Ship::default();
        ship.position = (7, 8, 9);
        ship.state = ShipState::Extracting(ExtractionInfo {
            mining_rate: BTreeMap::new(),
            time_fill_cargo: 0.0,
        });
        let pos = ship.stop_navigation().unwrap();
        assert_eq!(pos, (7, 8, 9));
        assert!(matches!(ship.state, ShipState::Idle));
    }

    #[test]
    fn test_stop_extraction_requires_extracting_state() {
        let mut ship = Ship::default();
        // Idle ship cannot stop an extraction
        assert!(matches!(
            ship.stop_extraction(),
            Err(Errcode::ShipNotExtracting)
        ));
        ship.state = ShipState::Extracting(ExtractionInfo {
            mining_rate: BTreeMap::new(),
            time_fill_cargo: 0.0,
        });
        assert!(ship.stop_extraction().is_ok());
        assert!(matches!(ship.state, ShipState::Idle));
    }

    #[test]
    fn test_set_travel_rejected_when_not_idle() {
        let mut ship = Ship::default();
        ship.state = ShipState::Extracting(ExtractionInfo {
            mining_rate: BTreeMap::new(),
            time_fill_cargo: 0.0,
        });
        assert!(matches!(
            ship.set_travel((10, 10, 10)),
            Err(Errcode::ShipNotIdle)
        ));
    }

    #[test]
    fn test_market_data_contains_id_and_price() {
        let ship = Ship::init_shipyard((0, 0, 0)).remove(0);
        let data = ship.market_data();
        assert_eq!(data["id"], serde_json::json!(ship.id));
        assert_eq!(data["price"], serde_json::json!(ship.compute_price()));
    }

    #[test]
    fn test_update_flight_reaches_destination() {
        let mut ship = Ship::default();
        ship.reactor_power = 1;
        ship.fuel_tank_capacity = 1e9;
        ship.fuel_tank = 1e9;
        ship.hull_resistance = 1e9;
        ship.crew
            .0
            .insert(1, CrewMember::from(CrewMemberType::Pilot));
        ship.pilot = Some(1);
        ship.update_perf_stats();

        // Axis-aligned travel so integer rounding lands exactly on target
        let cost = ship.set_travel((100, 0, 0)).unwrap();
        assert!(matches!(ship.state, ShipState::InFlight(_)));
        let finished = ship.update_flight(cost.duration);
        assert!(finished);
        assert_eq!(ship.position, (100, 0, 0));
        assert!(ship.fuel_tank < ship.fuel_tank_capacity);
    }

    #[test]
    fn test_update_flight_empty_tank_aborts() {
        let mut ship = Ship::default();
        ship.reactor_power = 1;
        ship.fuel_tank_capacity = 1e9;
        ship.fuel_tank = 1e9;
        ship.hull_resistance = 1e9;
        ship.crew
            .0
            .insert(1, CrewMember::from(CrewMemberType::Pilot));
        ship.pilot = Some(1);
        ship.update_perf_stats();
        ship.set_travel((100000, 0, 0)).unwrap();

        // Drain the tank then step: the flight aborts with an empty tank
        ship.fuel_tank = 0.0;
        assert!(ship.update_flight(0.001));
        assert_eq!(ship.fuel_tank, 0.0);
    }

    #[test]
    fn test_start_extraction_without_module_fails() {
        block_on(async {
            let mut ship = Ship::default();
            let planet = solid_planet();
            assert!(matches!(
                ship.start_extraction(&planet).await,
                Err(Errcode::CannotExtractWithoutModule)
            ));
        });
    }

    #[test]
    fn test_extraction_lifecycle() {
        block_on(async {
            let mut ship = mining_ship();
            let planet = solid_planet();
            let info = ship.start_extraction(&planet).await.unwrap();
            assert!(!info.mining_rate.is_empty());
            assert!(matches!(ship.state, ShipState::Extracting(_)));

            // Extracting twice is rejected (not idle)
            assert!(matches!(
                ship.start_extraction(&planet).await,
                Err(Errcode::ShipNotIdle)
            ));

            ship.update_extract(0.01);
            assert!(ship.cargo.usage > 0.0);

            assert!(ship.stop_extraction().is_ok());
            assert!(matches!(ship.state, ShipState::Idle));
        });
    }

    #[test]
    fn test_unload_cargo_to_station() {
        block_on(async {
            let station = Station::init(1, (0, 0, 0));
            let mut ship = Ship::default();
            ship.owner = 42;
            ship.cargo = ShipCargo::with_capacity(1000.0);
            ship.cargo.add_resource(&Resource::Iron, 10.0);

            let unloaded = ship
                .unload_cargo(&Resource::Iron, 4.0, &station)
                .await
                .unwrap();
            assert_eq!(unloaded, 4.0);
            assert_eq!(ship.cargo.resources[&Resource::Iron], 6.0);

            // unload_all empties the rest
            let all = ship.unload_all(&station).await.unwrap();
            assert_eq!(all[&Resource::Iron], 6.0);
        });
    }
}
