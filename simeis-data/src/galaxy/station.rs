use std::collections::BTreeMap;
use std::sync::Arc;

use mea::rwlock::RwLock;
use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::crew::{Crew, CrewId, CrewMember, CrewMemberType};
use crate::errors::Errcode;
use crate::industry::{IndustryUnit, IndustryUnitId, IndustryUnitType};
use crate::market::{fee_rate, Market, MarketTx};
use crate::player::{Player, PlayerId};
use crate::ship::cargo::ShipCargo;
use crate::ship::module::ShipModuleId;
use crate::ship::resources::Resource;
use crate::ship::upgrade::ShipUpgrade;
use crate::ship::Ship;
use crate::utils::ShardedLockedData;

use super::scan::ScanResult;
use super::{Galaxy, SpaceCoord};

const CARGO_BASE_PRICE: f64 = 2.0;
const CARGO_PRICE_INCDIV: f64 = 100.0;
pub const STATION_INIT_CARGO: f64 = 1000.0;

pub type StationId = u16;

#[derive(Serialize, Deserialize, Debug)]
pub struct StationInfo {
    pub id: StationId,
    pub position: SpaceCoord,
}

impl StationInfo {
    pub fn scan(_rank: u8, station: &Station) -> StationInfo {
        StationInfo {
            id: station.id,
            position: station.position,
        }
    }
}

#[derive(Default, Debug, Serialize)]
pub struct StationPlayerData {
    pub idle_crew: Crew,
    pub crew: Crew,
    pub trader: Option<CrewId>,
    pub cargo: ShipCargo,
    pub industry: BTreeMap<IndustryUnitId, IndustryUnit>,
}

impl StationPlayerData {
    pub fn new() -> StationPlayerData {
        StationPlayerData {
            cargo: ShipCargo::with_capacity(STATION_INIT_CARGO),
            ..Default::default()
        }
    }
}

pub struct Station {
    pub id: StationId,
    pub position: SpaceCoord,
    pub shipyard: RwLock<Vec<Ship>>,

    pub player_data: ShardedLockedData<PlayerId, Arc<RwLock<StationPlayerData>>>,
}

impl std::fmt::Debug for Station {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Station")
            .field("id", &self.id)
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

impl Station {
    pub fn init(id: u16, position: super::SpaceCoord) -> Station {
        Station {
            id,
            position,
            shipyard: RwLock::new(Ship::init_shipyard(position)),
            player_data: ShardedLockedData::new(100),
        }
    }

    pub async fn scan(&self, galaxy: &Galaxy) -> ScanResult {
        galaxy.scan_sector(1, &self.position).await
    }

    pub async fn cargo_price(&self, player: &PlayerId) -> f64 {
        let cap = if let Some(data) = self.player_data.clone_val(player).await {
            data.read().await.cargo.capacity
        } else {
            STATION_INIT_CARGO
        };
        CARGO_BASE_PRICE.powf((cap - STATION_INIT_CARGO) / CARGO_PRICE_INCDIV)
    }

    pub async fn buy_industry(
        &self,
        player: &mut Player,
        unit: IndustryUnitType,
    ) -> Result<(IndustryUnitId, f64), Errcode> {
        let cost = unit.get_price_buy();
        if player.money < cost {
            return Err(Errcode::NotEnoughMoney(player.money, cost));
        }
        self.ensure_has_player_data(&player.id).await;
        let pd = self.player_data.clone_val(&player.id).await.unwrap();
        let mut pd = pd.write().await;
        let unit = unit.new_unit();
        let unit_id = unit.id;
        pd.industry.insert(unit_id, unit);
        player.money -= cost;
        Ok((unit_id, cost))
    }

    pub async fn upgrade_industry(
        &self,
        player: &mut Player,
        id: &IndustryUnitId,
    ) -> Result<u8, Errcode> {
        self.ensure_has_player_data(&player.id).await;
        let pd = self.player_data.clone_val(&player.id).await.unwrap();
        let mut pd = pd.write().await;
        let Some(unit) = pd.industry.get_mut(id) else {
            return Err(Errcode::NoSuchIndustryUnit);
        };
        let cost = unit.price_next_rank();
        if cost > player.money {
            return Err(Errcode::NotEnoughMoney(player.money, cost));
        }
        player.money -= cost;
        unit.rank += 1;
        Ok(unit.rank)
    }

    pub async fn start_industry(
        &self,
        player: &PlayerId,
        id: &IndustryUnitId,
    ) -> Result<(), Errcode> {
        self.ensure_has_player_data(player).await;
        let pd = self.player_data.clone_val(player).await.unwrap();
        let mut pd = pd.write().await;
        let Some(unit) = pd.industry.get_mut(id) else {
            return Err(Errcode::NoSuchIndustryUnit);
        };
        unit.started = true;
        Ok(())
    }

    pub async fn stop_industry(
        &self,
        player: &PlayerId,
        id: &IndustryUnitId,
    ) -> Result<(), Errcode> {
        self.ensure_has_player_data(player).await;
        let pd = self.player_data.clone_val(player).await.unwrap();
        let mut pd = pd.write().await;
        let Some(unit) = pd.industry.get_mut(id) else {
            return Err(Errcode::NoSuchIndustryUnit);
        };
        unit.started = false;
        Ok(())
    }

    pub async fn buy_cargo(&self, player: &mut Player, amnt: &usize) -> Result<ShipCargo, Errcode> {
        let cost = (*amnt as f64) * self.cargo_price(&player.id).await;
        if cost > player.money {
            return Err(Errcode::NotEnoughMoney(player.money, cost));
        }
        player.money -= cost;
        self.ensure_has_player_data(&player.id).await;
        let pd = self.player_data.clone_val(&player.id).await.unwrap();
        let mut pd = pd.write().await;
        pd.cargo.capacity += *amnt as f64;
        Ok(pd.cargo.clone())
    }

    pub async fn add_cargo_cap(&self, player: &PlayerId, amnt: usize) -> ShipCargo {
        self.ensure_has_player_data(player).await;
        let pd = self.player_data.clone_val(player).await.unwrap();
        let mut pd = pd.write().await;
        pd.cargo.capacity += amnt as f64;
        pd.cargo.clone()
    }

    pub async fn assign_trader(&self, pid: &PlayerId, id: CrewId) -> Result<(), Errcode> {
        self.ensure_has_player_data(pid).await;
        let pd = self.player_data.clone_val(pid).await.unwrap();
        let mut pd = pd.write().await;
        let Some(cm) = pd.idle_crew.0.remove(&id) else {
            if pd.crew.0.contains_key(&id) {
                return Err(Errcode::CrewMemberNotIdle(id));
            } else {
                return Err(Errcode::CrewMemberNotFound(id));
            }
        };

        pd.crew.0.insert(id, cm);
        pd.trader = Some(id);
        Ok(())
    }

    pub async fn onboard_pilot(&self, ship: &mut Ship, id: &CrewId) -> Result<(), Errcode> {
        self.ensure_has_player_data(&ship.owner).await;
        let pd = self.player_data.clone_val(&ship.owner).await.unwrap();
        let mut pd = pd.write().await;
        let Some(cm) = pd.idle_crew.0.get(id) else {
            return Err(Errcode::CrewMemberNotIdle(*id));
        };
        if cm.member_type != CrewMemberType::Pilot {
            return Err(Errcode::WrongCrewType(CrewMemberType::Pilot));
        }
        ship.pilot = Some(*id);
        let pilot = pd.idle_crew.0.remove(id).unwrap();
        ship.crew.0.insert(*id, pilot);
        ship.update_perf_stats();
        Ok(())
    }

    pub async fn onboard_operator(
        &self,
        ship: &mut Ship,
        id: &CrewId,
        mod_id: &ShipModuleId,
    ) -> Result<(), Errcode> {
        self.ensure_has_player_data(&ship.owner).await;
        let cm = self
            .get_idle_crew(&ship.owner, id, CrewMemberType::Operator)
            .await?;
        let Some(module) = ship.modules.get_mut(mod_id) else {
            return Err(Errcode::NoSuchModule(*mod_id));
        };
        if !module.need(&cm.member_type) {
            return Err(Errcode::CrewNotNeeded);
        }
        module.operator = Some(*id);
        let pd = self.player_data.clone_val(&ship.owner).await.unwrap();
        let mut pd = pd.write().await;
        ship.crew.0.insert(*id, pd.idle_crew.0.remove(id).unwrap());
        Ok(())
    }

    pub async fn assign_crew_to_industry(
        &self,
        pid: &PlayerId,
        id: &CrewId,
        iid: &IndustryUnitId,
    ) -> Result<(), Errcode> {
        let cm = self
            .get_idle_crew(pid, id, CrewMemberType::Operator)
            .await?;
        let pd = self.player_data.clone_val(pid).await.unwrap();
        let mut pd = pd.write().await;
        let Some(industry) = pd.industry.get_mut(iid) else {
            return Err(Errcode::NoSuchIndustryUnit);
        };
        if !industry.need_crew_member(&cm.member_type) {
            return Err(Errcode::CrewNotNeeded);
        }
        industry.assign_operator(*id, &cm);
        let cm = pd.idle_crew.0.remove(id).unwrap();
        pd.crew.0.insert(*id, cm);
        Ok(())
    }

    pub async fn get_idle_crew(
        &self,
        pid: &PlayerId,
        id: &CrewId,
        ctype: CrewMemberType,
    ) -> Result<CrewMember, Errcode> {
        self.ensure_has_player_data(pid).await;
        let pd = self.player_data.clone_val(pid).await.unwrap();
        let pd = pd.read().await;
        let Some(cm) = pd.idle_crew.0.get(id) else {
            return Err(Errcode::CrewMemberNotIdle(*id));
        };
        if cm.member_type != ctype {
            return Err(Errcode::WrongCrewType(ctype));
        }
        Ok(cm.clone())
    }

    pub async fn buy_resource(
        &self,
        market: &Market,
        player: &PlayerId,
        resource: &Resource,
        amnt: f64,
    ) -> Result<MarketTx, Errcode> {
        self.ensure_has_player_data(player).await;
        let pd = self.player_data.clone_val(player).await.unwrap();
        let mut pd = pd.write().await;
        let Some(trader) = pd.trader else {
            return Err(Errcode::NoTraderAssigned);
        };
        let cm = pd.crew.0.get(&trader).unwrap();
        let can_cargo = pd.cargo.space_for(resource);
        let amnt = amnt.min(can_cargo);
        if amnt == 0.0 {
            return Err(Errcode::BuyNothing);
        }
        let tx = market.buy(cm, resource, amnt).await;
        let (r, a) = tx.added_cargo.unwrap();
        pd.cargo.add_resource(&r, a);
        Ok(tx)
    }

    pub async fn sell_resource(
        &self,
        market: &Market,
        player: &PlayerId,
        resource: &Resource,
        amnt: f64,
    ) -> Result<MarketTx, Errcode> {
        self.ensure_has_player_data(player).await;
        let pd = self.player_data.clone_val(player).await.unwrap();
        let mut pd = pd.write().await;
        let Some(trader) = pd.trader else {
            return Err(Errcode::NoTraderAssigned);
        };
        let cm = pd.crew.0.get(&trader).unwrap();
        let Some(can_cargo) = pd.cargo.resources.get(resource) else {
            return Err(Errcode::SellNothing);
        };
        let amnt = amnt.min(*can_cargo);
        if amnt <= 0.0 {
            return Err(Errcode::SellNothing);
        }
        let tx = market.sell(cm, resource, amnt).await;
        let (r, a) = tx.removed_cargo.unwrap();
        let unloaded = pd.cargo.unload(&r, a);
        debug_assert_eq!(unloaded, a);
        Ok(tx)
    }

    pub async fn refuel_ship(&self, ship: &mut Ship) -> Result<f64, Errcode> {
        if self.position != ship.position {
            return Err(Errcode::ShipNotInStation);
        }
        let Some(pd) = self.player_data.clone_val(&ship.owner).await else {
            return Err(Errcode::NoFuelInCargo);
        };
        let mut pd = pd.write().await;
        let Some(qty) = pd.cargo.resources.get(&Resource::Fuel) else {
            return Err(Errcode::NoFuelInCargo);
        };
        if *qty == 0.0 {
            return Err(Errcode::NoFuelInCargo);
        }
        debug_assert!(ship.fuel_tank >= 0.0);
        debug_assert!(ship.fuel_tank_capacity >= ship.fuel_tank);
        let needed = ship.fuel_tank_capacity - ship.fuel_tank;
        let unload = needed.min(*qty);
        let unloaded = pd.cargo.unload(&Resource::Fuel, unload);
        ship.fuel_tank += unloaded;
        debug_assert!(ship.fuel_tank_capacity >= ship.fuel_tank);
        Ok(unloaded)
    }

    pub async fn repair_ship(&self, ship: &mut Ship) -> Result<f64, Errcode> {
        if self.position != ship.position {
            return Err(Errcode::ShipNotInStation);
        }
        let Some(pd) = self.player_data.clone_val(&ship.owner).await else {
            return Err(Errcode::NoHullInCargo);
        };
        let mut pd = pd.write().await;
        let Some(qty) = pd.cargo.resources.get(&Resource::Hull) else {
            return Err(Errcode::NoHullInCargo);
        };
        if *qty == 0.0 {
            return Err(Errcode::NoHullInCargo);
        }
        debug_assert!(ship.hull_resistance >= ship.hull_decay);

        let amnt = ship.hull_decay.min(*qty);
        if amnt == 0.0 {
            return Ok(0.0);
        }
        let unloaded = pd.cargo.unload(&Resource::Hull, amnt);
        ship.hull_decay -= unloaded;
        debug_assert!(
            ship.hull_resistance >= ship.hull_decay,
            "{} < {}",
            ship.hull_resistance,
            ship.hull_decay
        );
        debug_assert!(ship.hull_decay >= 0.0, "{}", ship.hull_decay);
        debug_assert!(unloaded >= 0.0, "{}", unloaded);
        Ok(unloaded)
    }

    pub fn get_ship_upgrade_price(&self, _ship: &Ship, upgrade: &ShipUpgrade) -> f64 {
        upgrade.get_price()
    }

    pub async fn get_cargo_potential_price(&self, id: &PlayerId) -> f64 {
        let Some(pd) = self.player_data.clone_val(id).await else {
            return 0.0;
        };
        let pd = pd.read().await;
        pd.cargo
            .resources
            .iter()
            .map(|(r, amnt)| r.base_price() * amnt)
            .sum()
    }

    pub async fn add_resource(&self, id: &PlayerId, resource: &Resource, amnt: f64) -> f64 {
        self.ensure_has_player_data(id).await;
        let pd = self.player_data.clone_val(id).await.unwrap();
        let mut pd = pd.write().await;
        pd.cargo.add_resource(resource, amnt)
    }

    pub async fn buy_ship(&self, index: usize) -> Ship {
        // Ship starters, always keep them
        let mut ship = if index < 3 {
            let shipyard = self.shipyard.read().await;
            shipyard.get(index).unwrap().clone()
        } else {
            let mut shipyard = self.shipyard.write().await;
            let ship = shipyard.remove(index);
            shipyard.push(Ship::random(self.position));
            ship
        };
        ship.update_perf_stats();
        ship.fuel_tank = ship.fuel_tank_capacity;
        ship
    }

    pub async fn ensure_has_player_data(&self, id: &PlayerId) {
        if !self.player_data.contains_key(id).await {
            let pd = Arc::new(RwLock::new(StationPlayerData::new()));
            self.player_data.insert(*id, pd).await;
        }
    }

    pub async fn sum_all_wages(&self, id: &PlayerId) -> f64 {
        let Some(pd) = self.player_data.clone_val(id).await else {
            return 0.0;
        };
        let pd = pd.read().await;
        pd.crew.sum_wages() + pd.idle_crew.sum_wages()
    }

    pub async fn upgrade_station_crew(
        &self,
        id: &PlayerId,
        money: &mut f64,
        crew: &CrewId,
    ) -> Result<(f64, u8), Errcode> {
        let Some(pd) = self.player_data.clone_val(id).await else {
            return Err(Errcode::CrewMemberNotFound(*crew));
        };
        let mut pd = pd.write().await;

        let Some(cm) = pd.crew.0.get_mut(crew) else {
            return Err(Errcode::CrewMemberNotFound(*crew));
        };
        let price = cm.price_next_rank();
        if price > *money {
            return Err(Errcode::NotEnoughMoney(*money, price));
        }
        *money -= price;
        cm.rank += 1;
        Ok((price, cm.rank))
    }

    pub async fn hire_crew(&self, id: &PlayerId, crewtype: CrewMemberType) -> CrewId {
        let crewid = rand::rng().random();
        let member = CrewMember::from(crewtype);

        self.ensure_has_player_data(id).await;
        let pd = self.player_data.clone_val(id).await.unwrap();
        let mut pd = pd.write().await;
        pd.idle_crew.0.insert(crewid, member);
        crewid
    }

    pub async fn fire_crew(&self, id: &PlayerId, crewid: &CrewId) -> Result<CrewMember, Errcode> {
        self.ensure_has_player_data(id).await;
        let pd = self.player_data.clone_val(id).await.unwrap();
        let mut pd = pd.write().await;
        let Some(cm) = pd.idle_crew.0.remove(crewid) else {
            return Err(Errcode::CrewMemberNotFound(*crewid));
        };
        Ok(cm)
    }

    pub async fn upgr_trader_price(&self, id: &PlayerId) -> Option<f64> {
        let pd = self.player_data.clone_val(id).await?;
        let pd = pd.read().await;
        pd.trader.map(|trader| {
            let cm = pd.crew.0.get(&trader).unwrap();
            cm.price_next_rank()
        })
    }

    pub async fn clone_cargo(&self, id: &PlayerId) -> ShipCargo {
        let Some(pd) = self.player_data.clone_val(id).await else {
            return ShipCargo::with_capacity(STATION_INIT_CARGO);
        };
        let pd = pd.read().await;
        pd.cargo.clone()
    }

    pub async fn get_fee_rate(&self, id: &PlayerId) -> Result<f64, Errcode> {
        let Some(pd) = self.player_data.clone_val(id).await else {
            return Err(Errcode::NoTraderAssigned);
        };
        let pd = pd.read().await;
        let Some(trader) = pd.trader else {
            return Err(Errcode::NoTraderAssigned);
        };
        let cm = pd.crew.0.get(&trader).unwrap();
        Ok(fee_rate(cm.rank))
    }

    pub async fn to_json(&self, id: &PlayerId) -> serde_json::Value {
        if let Some(pd) = self.player_data.clone_val(id).await {
            let pd = pd.read().await;
            self._to_json(&pd)
        } else {
            let pd = StationPlayerData::new();
            self._to_json(&pd)
        }
    }

    fn _to_json(&self, data: &StationPlayerData) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "position": self.position,
            "crew": data.crew,
            "cargo": data.cargo,
            "idle_crew": data.idle_crew,
            "trader": data.trader,
        })
    }

    pub async fn update_crafting(&self, tdelta: f64, id: &PlayerId) {
        let Some(pd) = self.player_data.clone_val(id).await else {
            return;
        };
        let mut pd = pd.write().await;
        let all_industry = pd.industry.clone();
        for (_, industry) in all_industry.iter() {
            if let Some(ratio) = industry.can_work(&tdelta, &pd.cargo.resources) {
                let t = tdelta * ratio;
                industry.work(t, &mut pd.cargo.resources);
            }
        }
    }

    pub async fn get_industry_production(
        &self,
        pid: &PlayerId,
        id: IndustryUnitId,
    ) -> Result<(Vec<(Resource, f64)>, Vec<(Resource, f64)>), Errcode> {
        let Some(pd) = self.player_data.clone_val(pid).await else {
            return Err(Errcode::NoSuchIndustryUnit);
        };

        let pd = pd.read().await;
        let Some(industry) = pd.industry.get(&id) else {
            return Err(Errcode::NoSuchIndustryUnit);
        };

        let Some(opid) = industry.operator else {
            return Ok((vec![], vec![]));
        };
        let op = pd.crew.0.get(&opid).unwrap();

        let inputs = industry.input(op.rank);
        let outputs = industry.output(op.rank);
        Ok((inputs, outputs))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;
    use crate::player::Player;
    use crate::ship::module::ShipModuleType;
    use crate::tests::block_on;

    const POS: SpaceCoord = (100, 200, 300);

    fn station() -> Arc<Station> {
        Arc::new(Station::init(1, POS))
    }

    fn player(station: Arc<Station>) -> Player {
        Player::new((station.id, station), "tester".to_string())
    }

    fn ship_owned_by(player: &Player) -> Ship {
        let mut ship = Ship::default();
        ship.owner = player.id;
        ship.position = POS;
        ship
    }

    #[test]
    fn test_init_fields() {
        let st = Station::init(42, POS);
        assert_eq!(st.id, 42);
        assert_eq!(st.position, POS);
    }

    #[test]
    fn test_ensure_has_player_data_idempotent() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            assert!(!st.player_data.contains_key(&p.id).await);
            st.ensure_has_player_data(&p.id).await;
            st.ensure_has_player_data(&p.id).await;
            assert!(st.player_data.contains_key(&p.id).await);
        });
    }

    #[test]
    fn test_cargo_price_default_is_one() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            // With the default capacity the exponent is 0 -> price 1.0
            assert_eq!(st.cargo_price(&p.id).await, 1.0);
        });
    }

    #[test]
    fn test_hire_and_fire_crew() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            let id = st.hire_crew(&p.id, CrewMemberType::Operator).await;
            let cm = st.fire_crew(&p.id, &id).await.unwrap();
            assert_eq!(cm.member_type, CrewMemberType::Operator);
            // Firing the same member again fails
            assert!(matches!(
                st.fire_crew(&p.id, &id).await,
                Err(Errcode::CrewMemberNotFound(_))
            ));
        });
    }

    #[test]
    fn test_buy_industry_success_and_insufficient_funds() {
        block_on(async {
            let st = station();
            let mut p = player(st.clone());
            let before = p.money;
            let (uid, cost) = st
                .buy_industry(&mut p, IndustryUnitType::SimpleFuelRefinery)
                .await
                .unwrap();
            assert_eq!(cost, IndustryUnitType::SimpleFuelRefinery.get_price_buy());
            assert!((p.money - (before - cost)).abs() < 1e-9);

            // Drain the wallet, the next purchase must fail
            p.money = 0.0;
            assert!(matches!(
                st.buy_industry(&mut p, IndustryUnitType::AdvancedFuelRefinery)
                    .await,
                Err(Errcode::NotEnoughMoney(_, _))
            ));
            // The first unit is still present
            assert!(st.get_industry_production(&p.id, uid).await.is_ok());
        });
    }

    #[test]
    fn test_start_stop_and_upgrade_industry() {
        block_on(async {
            let st = station();
            let mut p = player(st.clone());
            let (uid, _) = st
                .buy_industry(&mut p, IndustryUnitType::SimpleHullFoundry)
                .await
                .unwrap();

            assert!(st.start_industry(&p.id, &uid).await.is_ok());
            assert!(st.stop_industry(&p.id, &uid).await.is_ok());

            let rank = st.upgrade_industry(&mut p, &uid).await.unwrap();
            assert_eq!(rank, 2);

            // Unknown units are rejected
            assert!(matches!(
                st.start_industry(&p.id, &999_999).await,
                Err(Errcode::NoSuchIndustryUnit)
            ));
            assert!(matches!(
                st.upgrade_industry(&mut p, &999_999).await,
                Err(Errcode::NoSuchIndustryUnit)
            ));
        });
    }

    #[test]
    fn test_assign_trader_errors_and_success() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            // Unknown crew member
            assert!(matches!(
                st.assign_trader(&p.id, 12345).await,
                Err(Errcode::CrewMemberNotFound(_))
            ));
            let trader = st.hire_crew(&p.id, CrewMemberType::Trader).await;
            assert!(st.assign_trader(&p.id, trader).await.is_ok());
            // The fee rate is now available
            assert!(st.get_fee_rate(&p.id).await.is_ok());
            assert!(st.upgr_trader_price(&p.id).await.is_some());
        });
    }

    #[test]
    fn test_buy_and_add_cargo_capacity() {
        block_on(async {
            let st = station();
            let mut p = player(st.clone());
            let cargo = st.buy_cargo(&mut p, &100).await.unwrap();
            assert!((cargo.capacity - (STATION_INIT_CARGO + 100.0)).abs() < 1e-9);
            let cargo = st.add_cargo_cap(&p.id, 50).await;
            assert!((cargo.capacity - (STATION_INIT_CARGO + 150.0)).abs() < 1e-9);
        });
    }

    #[test]
    fn test_buy_sell_resource_requires_trader() {
        block_on(async {
            let st = station();
            let market = Market::init();
            let p = player(st.clone());
            // No trader assigned yet
            assert!(matches!(
                st.buy_resource(&market, &p.id, &Resource::Iron, 5.0).await,
                Err(Errcode::NoTraderAssigned)
            ));
            assert!(matches!(
                st.sell_resource(&market, &p.id, &Resource::Iron, 5.0).await,
                Err(Errcode::NoTraderAssigned)
            ));

            let trader = st.hire_crew(&p.id, CrewMemberType::Trader).await;
            st.assign_trader(&p.id, trader).await.unwrap();

            // Buy then sell the same resource
            let tx = st
                .buy_resource(&market, &p.id, &Resource::Iron, 5.0)
                .await
                .unwrap();
            assert_eq!(tx.added_cargo.unwrap().0, Resource::Iron);

            let tx = st
                .sell_resource(&market, &p.id, &Resource::Iron, 2.0)
                .await
                .unwrap();
            assert_eq!(tx.removed_cargo.unwrap().0, Resource::Iron);

            // Selling a resource we don't hold fails
            assert!(matches!(
                st.sell_resource(&market, &p.id, &Resource::Gold, 1.0).await,
                Err(Errcode::SellNothing)
            ));
        });
    }

    #[test]
    fn test_refuel_ship() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            let mut ship = ship_owned_by(&p);
            ship.fuel_tank_capacity = 100.0;
            ship.fuel_tank = 0.0;

            // No fuel in cargo yet
            assert!(matches!(
                st.refuel_ship(&mut ship).await,
                Err(Errcode::NoFuelInCargo)
            ));

            st.add_resource(&p.id, &Resource::Fuel, 30.0).await;
            let unloaded = st.refuel_ship(&mut ship).await.unwrap();
            assert_eq!(unloaded, 30.0);
            assert_eq!(ship.fuel_tank, 30.0);

            // A ship away from the station cannot refuel
            ship.position = (0, 0, 0);
            assert!(matches!(
                st.refuel_ship(&mut ship).await,
                Err(Errcode::ShipNotInStation)
            ));
        });
    }

    #[test]
    fn test_repair_ship() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            let mut ship = ship_owned_by(&p);
            ship.hull_resistance = 1000.0;
            ship.hull_decay = 40.0;

            assert!(matches!(
                st.repair_ship(&mut ship).await,
                Err(Errcode::NoHullInCargo)
            ));

            st.add_resource(&p.id, &Resource::Hull, 25.0).await;
            let repaired = st.repair_ship(&mut ship).await.unwrap();
            assert_eq!(repaired, 25.0);
            assert_eq!(ship.hull_decay, 15.0);
        });
    }

    #[test]
    fn test_onboard_pilot_and_operator() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            let mut ship = ship_owned_by(&p);

            // Pilot
            let pilot = st.hire_crew(&p.id, CrewMemberType::Pilot).await;
            st.onboard_pilot(&mut ship, &pilot).await.unwrap();
            assert_eq!(ship.pilot, Some(pilot));
            assert!(ship.crew.0.contains_key(&pilot));

            // Operator on a module
            ship.modules.insert(1, ShipModuleType::Miner.new_module());
            let op = st.hire_crew(&p.id, CrewMemberType::Operator).await;
            st.onboard_operator(&mut ship, &op, &1).await.unwrap();
            assert_eq!(ship.modules.get(&1).unwrap().operator, Some(op));
        });
    }

    #[test]
    fn test_onboard_pilot_wrong_type_rejected() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            let mut ship = ship_owned_by(&p);
            let op = st.hire_crew(&p.id, CrewMemberType::Operator).await;
            assert!(matches!(
                st.onboard_pilot(&mut ship, &op).await,
                Err(Errcode::WrongCrewType(_))
            ));
        });
    }

    #[test]
    fn test_assign_crew_to_industry() {
        block_on(async {
            let st = station();
            let mut p = player(st.clone());
            let (uid, _) = st
                .buy_industry(&mut p, IndustryUnitType::SimpleFuelRefinery)
                .await
                .unwrap();
            let op = st.hire_crew(&p.id, CrewMemberType::Operator).await;
            assert!(st.assign_crew_to_industry(&p.id, &op, &uid).await.is_ok());
            // After assigning an operator, production recipes are non-empty
            let (inputs, outputs) = st.get_industry_production(&p.id, uid).await.unwrap();
            assert!(!inputs.is_empty());
            assert!(!outputs.is_empty());
        });
    }

    #[test]
    fn test_upgrade_station_crew() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            let trader = st.hire_crew(&p.id, CrewMemberType::Trader).await;
            st.assign_trader(&p.id, trader).await.unwrap();

            let mut money = 1_000_000.0;
            let (price, rank) = st
                .upgrade_station_crew(&p.id, &mut money, &trader)
                .await
                .unwrap();
            assert!(price > 0.0);
            assert_eq!(rank, 2);
            assert!((money - (1_000_000.0 - price)).abs() < 1e-9);

            // Unknown crew member fails
            assert!(matches!(
                st.upgrade_station_crew(&p.id, &mut money, &999).await,
                Err(Errcode::CrewMemberNotFound(_))
            ));
        });
    }

    #[test]
    fn test_buy_ship_starters_are_kept() {
        block_on(async {
            let st = station();
            let initial_len = st.shipyard.read().await.len();
            let ship = st.buy_ship(0).await;
            // Buying a starter (index < 3) does not shrink the shipyard
            assert_eq!(st.shipyard.read().await.len(), initial_len);
            assert_eq!(ship.fuel_tank, ship.fuel_tank_capacity);
        });
    }

    #[test]
    fn test_clone_cargo_and_potential_price() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            // Default cargo for an unknown player
            let cargo = st.clone_cargo(&p.id).await;
            assert_eq!(cargo.capacity, STATION_INIT_CARGO);
            assert_eq!(st.get_cargo_potential_price(&p.id).await, 0.0);

            st.add_resource(&p.id, &Resource::Gold, 10.0).await;
            let price = st.get_cargo_potential_price(&p.id).await;
            assert!((price - 10.0 * Resource::Gold.base_price()).abs() < 1e-9);
        });
    }

    #[test]
    fn test_sum_all_wages() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            assert_eq!(st.sum_all_wages(&p.id).await, 0.0);
            st.hire_crew(&p.id, CrewMemberType::Pilot).await;
            assert!(st.sum_all_wages(&p.id).await > 0.0);
        });
    }

    #[test]
    fn test_to_json_contains_station_id() {
        block_on(async {
            let st = station();
            let p = player(st.clone());
            let json = st.to_json(&p.id).await;
            assert_eq!(json["id"], serde_json::json!(st.id));
            assert_eq!(json["position"], serde_json::json!(st.position));
        });
    }

    #[test]
    fn test_update_crafting_consumes_and_produces() {
        block_on(async {
            let st = station();
            let mut p = player(st.clone());
            let (uid, _) = st
                .buy_industry(&mut p, IndustryUnitType::SimpleFuelRefinery)
                .await
                .unwrap();
            let op = st.hire_crew(&p.id, CrewMemberType::Operator).await;
            st.assign_crew_to_industry(&p.id, &op, &uid).await.unwrap();
            st.start_industry(&p.id, &uid).await.unwrap();

            // Make room before stocking, otherwise the cargo fills up with the
            // first resource and the others can't be added.
            st.add_cargo_cap(&p.id, 100_000).await;
            for res in [
                Resource::Hydrogen,
                Resource::Oxygen,
                Resource::Carbon,
                Resource::Water,
            ] {
                st.add_resource(&p.id, &res, 100.0).await;
            }
            st.update_crafting(1.0, &p.id).await;

            let cargo = st.clone_cargo(&p.id).await;
            assert!(cargo.resources.get(&Resource::Fuel).copied().unwrap_or(0.0) > 0.0);
        });
    }

    #[test]
    fn test_get_ship_upgrade_price_delegates() {
        let st = Station::init(1, POS);
        let ship = Ship::default();
        assert_eq!(
            st.get_ship_upgrade_price(&ship, &ShipUpgrade::Shield),
            ShipUpgrade::Shield.get_price()
        );
    }
}
