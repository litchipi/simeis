use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::Hasher;
use std::sync::Arc;
use std::time::Instant;

use rand::Rng;

use crate::crew::CrewId;
use crate::errors::Errcode;
use crate::galaxy::station::{Station, StationId};
use crate::ship::cargo::ShipCargo;
use crate::ship::module::{ShipModuleId, ShipModuleType};
use crate::ship::upgrade::ShipUpgrade;
use crate::ship::{Ship, ShipId};
use crate::syslog::{SyslogEvent, SyslogRecv};

const INIT_MONEY: f64 = 72000.0;

pub type PlayerId = u64;
pub type PlayerKey = [u8; 128];

// Game state for a single player
pub struct Player {
    pub created: Instant,
    pub id: PlayerId,
    pub key: PlayerKey,
    pub score: f64,
    pub lost: bool,

    pub name: String,
    pub money: f64,
    pub costs: f64,

    pub stations: BTreeMap<StationId, Arc<Station>>,
    pub ships: BTreeMap<ShipId, Ship>,
}

impl Player {
    pub fn new(initstation: (StationId, Arc<Station>), name: String) -> Player {
        let mut hasher = DefaultHasher::new();
        hasher.write(name.as_bytes());
        let mut rng = rand::rng();
        let mut randbytes = [0; 128];
        rng.fill_bytes(&mut randbytes);

        #[allow(unused_mut)]
        let mut money = INIT_MONEY;

        #[cfg(feature = "testing")]
        if name.starts_with("test-rich") {
            money *= 10000.0;
        }
        let mut stations = BTreeMap::new();
        stations.insert(initstation.0, initstation.1);
        Player {
            created: Instant::now(),
            key: randbytes,
            id: hasher.finish(),
            lost: false,

            money,
            score: 0.0,
            costs: 0.0,

            name,
            stations,
            ships: BTreeMap::new(),
        }
    }

    pub fn get_ship<'a>(&'a self, id: &ShipId) -> Result<&'a Ship, Errcode> {
        self.ships.get(id).ok_or(Errcode::ShipNotFound(*id))
    }

    pub fn get_ship_mut<'a>(&'a mut self, id: &ShipId) -> Result<&'a mut Ship, Errcode> {
        self.ships.get_mut(id).ok_or(Errcode::ShipNotFound(*id))
    }

    #[inline]
    pub fn ship_in_station(&self, ship: &ShipId, station: &StationId) -> Result<bool, Errcode> {
        let ship = self.get_ship(ship)?;
        let Some(station) = self.stations.get(station) else {
            return Err(Errcode::NoSuchStation(*station));
        };
        Ok(ship.position == station.position)
    }

    //// Interfaces for game

    pub async fn update_costs(&mut self) {
        self.costs = 0.0;

        for station in self.stations.values() {
            // Deadlock because of this
            self.costs += station.sum_all_wages(&self.id).await;
        }
        self.costs += self
            .ships
            .values()
            .map(|ship| ship.crew.sum_wages())
            .sum::<f64>();
    }

    pub async fn update_money(&mut self, syslog: &SyslogRecv, tdelta: f64) {
        let before = self.money < (self.costs * 60.0);
        self.money -= self.costs * tdelta;
        let after = self.money < (self.costs * 60.0);
        if after && !before {
            let tleft = std::time::Duration::from_secs_f64(self.money / self.costs);
            syslog.event(self.id, SyslogEvent::LowFunds(tleft)).await;
        }
        if self.money < 0.0 && !self.lost {
            self.lost = true;
            syslog.event(self.id, SyslogEvent::GameLost).await;
        }
    }

    pub async fn buy_ship(
        &mut self,
        station_id: &StationId,
        ship_id: &ShipId,
    ) -> Result<ShipId, Errcode> {
        let Some(station) = self.stations.get(station_id) else {
            return Err(Errcode::NoSuchStation(*station_id));
        };

        let ship_opt = {
            let mut data = None;
            let shipyard = station.shipyard.read().await;
            for (n, ship) in shipyard.iter().enumerate() {
                if &ship.id == ship_id {
                    data = Some((n, ship.compute_price()));
                }
            }
            data
        };

        let Some((index, price)) = ship_opt else {
            return Err(Errcode::ShipNotFound(*ship_id));
        };

        if price > self.money {
            return Err(Errcode::NotEnoughMoney(self.money, price));
        }

        let mut ship = station.buy_ship(index).await;
        let ship_id = ship.id;
        ship.owner = self.id;
        self.money -= price;
        self.ships.insert(ship_id, ship);
        Ok(ship_id)
    }

    pub async fn buy_ship_module(
        &mut self,
        station_id: &StationId,
        ship_id: &ShipId,
        modtype: ShipModuleType,
    ) -> Result<ShipModuleId, Errcode> {
        if !self.ship_in_station(ship_id, station_id)? {
            return Err(Errcode::ShipNotInStation);
        }
        let ship = self.ships.get_mut(ship_id).unwrap();

        let price = modtype.get_price_buy();
        if self.money < price {
            return Err(Errcode::NotEnoughMoney(self.money, price));
        }
        self.money -= price;
        let id = (ship.modules.len() + 1) as ShipModuleId;
        ship.modules.insert(id, modtype.new_module());
        Ok(id)
    }

    pub async fn buy_ship_upgrade(
        &mut self,
        station: &StationId,
        ship_id: &ShipId,
        upgrade: &ShipUpgrade,
    ) -> Result<f64, Errcode> {
        let ship = self
            .ships
            .get_mut(ship_id)
            .ok_or(Errcode::ShipNotFound(*ship_id))?;
        let Some(station) = self.stations.get(station).cloned() else {
            return Err(Errcode::NoSuchStation(*station));
        };

        let price = station.get_ship_upgrade_price(ship, upgrade);
        if price > self.money {
            return Err(Errcode::NotEnoughMoney(self.money, price));
        }

        self.money -= price;
        upgrade.install(ship);
        Ok(price)
    }

    pub async fn buy_ship_module_upgrade(
        &mut self,
        station_id: &StationId,
        ship_id: &ShipId,
        mod_id: &ShipModuleId,
    ) -> Result<(f64, u8), Errcode> {
        if !self.ship_in_station(ship_id, station_id)? {
            return Err(Errcode::ShipNotInStation);
        }
        // SAFETY Checked on the function above
        let ship = self.ships.get_mut(ship_id).unwrap();
        let Some(ref mut module) = ship.modules.get_mut(mod_id) else {
            return Err(Errcode::NoSuchModule(*mod_id));
        };
        let price = module.price_next_rank();
        if price > self.money {
            return Err(Errcode::NotEnoughMoney(self.money, price));
        }

        self.money -= price;
        module.rank += 1;

        Ok((price, module.rank))
    }

    pub async fn upgrade_ship_crew(
        &mut self,
        station_id: &StationId,
        ship_id: &ShipId,
        crew_id: &CrewId,
    ) -> Result<(f64, u8), Errcode> {
        if !self.ship_in_station(ship_id, station_id)? {
            return Err(Errcode::ShipNotInStation);
        };
        // SAFETY Checked in function above
        let ship = self.ships.get_mut(ship_id).unwrap();
        let res = {
            let Some(ref mut cm) = ship.crew.0.get_mut(crew_id) else {
                return Err(Errcode::CrewMemberNotFound(*crew_id));
            };

            let price = cm.price_next_rank();
            if price > self.money {
                return Err(Errcode::NotEnoughMoney(self.money, price));
            }

            self.money -= price;
            cm.rank += 1;
            (price, cm.rank)
        };
        ship.update_perf_stats();
        Ok(res)
    }

    pub async fn upgrade_station_crew(
        &mut self,
        station_id: &StationId,
        crew_id: &CrewId,
    ) -> Result<(f64, u8), Errcode> {
        let Some(station) = self.stations.get(station_id) else {
            return Err(Errcode::NoSuchStation(*station_id));
        };
        let res = station
            .upgrade_station_crew(&self.id, &mut self.money, crew_id)
            .await;
        match res {
            Ok(v) => {
                self.update_costs().await;
                Ok(v)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn buy_station_cargo(
        &mut self,
        station_id: &StationId,
        amnt: usize,
    ) -> Result<ShipCargo, Errcode> {
        let Some(station) = self.stations.get_mut(station_id) else {
            return Err(Errcode::NoSuchStation(*station_id));
        };
        let cost = (amnt as f64) * station.cargo_price(&self.id).await;
        if cost > self.money {
            return Err(Errcode::NotEnoughMoney(self.money, cost));
        }
        self.money -= cost;
        Ok(station.add_cargo_cap(&self.id, amnt).await)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;
    use crate::crew::CrewMemberType;
    use crate::galaxy::station::Station;
    use crate::galaxy::SpaceCoord;
    use crate::syslog::SyslogSend;
    use crate::tests::block_on;

    const POS: SpaceCoord = (10, 20, 30);

    fn new_player() -> (Arc<Station>, Player) {
        let station = Arc::new(Station::init(1, POS));
        let player = Player::new((station.id, station.clone()), "tester".to_string());
        (station, player)
    }

    #[test]
    fn test_new_player_initial_state() {
        let (_st, p) = new_player();
        assert_eq!(p.money, INIT_MONEY);
        assert_eq!(p.score, 0.0);
        assert!(!p.lost);
        assert_eq!(p.name, "tester");
        assert_eq!(p.stations.len(), 1);
        assert!(p.ships.is_empty());
    }

    #[test]
    fn test_get_ship_not_found() {
        let (_st, p) = new_player();
        assert!(matches!(p.get_ship(&123), Err(Errcode::ShipNotFound(_))));
    }

    #[test]
    fn test_ship_in_station_checks_position() {
        let (st, mut p) = new_player();
        let mut ship = Ship::default();
        ship.id = 7;
        ship.position = POS;
        p.ships.insert(7, ship);
        assert!(p.ship_in_station(&7, &st.id).unwrap());

        // Move the ship away from the station
        p.ships.get_mut(&7).unwrap().position = (0, 0, 0);
        assert!(!p.ship_in_station(&7, &st.id).unwrap());

        // Unknown station
        assert!(matches!(
            p.ship_in_station(&7, &999),
            Err(Errcode::NoSuchStation(_))
        ));
    }

    #[test]
    fn test_buy_ship_success_and_too_expensive() {
        block_on(async {
            let (st, mut p) = new_player();
            let ship_id = st.shipyard.read().await.first().unwrap().id;
            let bought = p.buy_ship(&st.id, &ship_id).await.unwrap();
            assert!(p.ships.contains_key(&bought));
            assert!(p.money < INIT_MONEY);
            assert_eq!(p.ships.get(&bought).unwrap().owner, p.id);

            // Unknown ship id
            assert!(matches!(
                p.buy_ship(&st.id, &424242).await,
                Err(Errcode::ShipNotFound(_))
            ));

            // Not enough money for another ship
            p.money = 0.0;
            let other = st.shipyard.read().await.get(1).unwrap().id;
            assert!(matches!(
                p.buy_ship(&st.id, &other).await,
                Err(Errcode::NotEnoughMoney(_, _))
            ));
        });
    }

    #[test]
    fn test_buy_ship_module_requires_ship_in_station() {
        block_on(async {
            let (st, mut p) = new_player();
            let ship_id = st.shipyard.read().await.first().unwrap().id;
            let ship_id = p.buy_ship(&st.id, &ship_id).await.unwrap();
            // Bought ships spawn at the station position
            let mod_id = p
                .buy_ship_module(&st.id, &ship_id, ShipModuleType::Miner)
                .await
                .unwrap();
            assert!(p.ships.get(&ship_id).unwrap().modules.contains_key(&mod_id));
        });
    }

    #[test]
    fn test_buy_ship_upgrade_reduces_money_and_installs() {
        block_on(async {
            let (st, mut p) = new_player();
            let ship_id = st.shipyard.read().await.first().unwrap().id;
            let ship_id = p.buy_ship(&st.id, &ship_id).await.unwrap();
            let cap_before = p.ships.get(&ship_id).unwrap().cargo.capacity;
            let money_before = p.money;

            let price = p
                .buy_ship_upgrade(&st.id, &ship_id, &ShipUpgrade::CargoExpansion)
                .await
                .unwrap();
            assert!((p.money - (money_before - price)).abs() < 1e-9);
            assert!(p.ships.get(&ship_id).unwrap().cargo.capacity > cap_before);
        });
    }

    #[test]
    fn test_upgrade_ship_crew() {
        block_on(async {
            let (st, mut p) = new_player();
            let ship_id = st.shipyard.read().await.first().unwrap().id;
            let ship_id = p.buy_ship(&st.id, &ship_id).await.unwrap();
            let ship = p.ships.get_mut(&ship_id).unwrap();
            ship.crew
                .0
                .insert(1, crate::crew::CrewMember::from(CrewMemberType::Pilot));
            ship.pilot = Some(1);
            p.money = 1_000_000.0;

            let (price, rank) = p.upgrade_ship_crew(&st.id, &ship_id, &1).await.unwrap();
            assert!(price > 0.0);
            assert_eq!(rank, 2);

            // Unknown crew member
            assert!(matches!(
                p.upgrade_ship_crew(&st.id, &ship_id, &999).await,
                Err(Errcode::CrewMemberNotFound(_))
            ));
        });
    }

    #[test]
    fn test_buy_station_cargo() {
        block_on(async {
            let (st, mut p) = new_player();
            let before = p.money;
            let cargo = p.buy_station_cargo(&st.id, 100).await.unwrap();
            assert!(cargo.capacity > crate::galaxy::station::STATION_INIT_CARGO);
            assert!(p.money <= before);
        });
    }

    #[test]
    fn test_update_money_decreases_by_costs() {
        block_on(async {
            let (_st, mut p) = new_player();
            let (_send, recv) = SyslogSend::channel();
            p.costs = 100.0;
            let before = p.money;
            p.update_money(&recv, 2.0).await;
            assert!((p.money - (before - 200.0)).abs() < 1e-9);
            assert!(!p.lost);
        });
    }

    #[test]
    fn test_update_money_emits_low_funds_on_threshold_crossing() {
        block_on(async {
            let (_st, mut p) = new_player();
            let (_send, recv) = SyslogSend::channel();
            p.costs = 100.0;
            // Threshold is costs * 60 = 6000. Start just above it, drop below.
            p.money = 6050.0;
            p.update_money(&recv, 1.0).await; // money -> 5950
            assert!(p.money < 6000.0);
            assert!(!p.lost);
            // The low-funds event was pushed to the syslog fifo
            let fifo = recv.fifo.read().await;
            assert!(fifo.clone_val(&p.id).await.is_some());
        });
    }

    #[test]
    fn test_update_money_marks_lost_when_negative() {
        block_on(async {
            let (_st, mut p) = new_player();
            let (_send, recv) = SyslogSend::channel();
            p.money = 50.0;
            p.costs = 100.0;
            p.update_money(&recv, 1.0).await; // money becomes -50
            assert!(p.money < 0.0);
            assert!(p.lost);
        });
    }

    #[test]
    fn test_update_costs_sums_ship_crew_wages() {
        block_on(async {
            let (_st, mut p) = new_player();
            let mut ship = Ship::default();
            ship.id = 5;
            ship.crew
                .0
                .insert(1, crate::crew::CrewMember::from(CrewMemberType::Pilot));
            p.ships.insert(5, ship);
            p.update_costs().await;
            assert!(p.costs > 0.0);
        });
    }
}
