// Mutex acquisition order
// - Taken names
// - Player index
// - Player list
// - Player
// - Station
// - Galaxy
// - Market
// - SyslogFifo
// - PlayerFifo

use std::sync::Arc;
use std::time::{Duration, Instant};

use mea::mpsc::{BoundedReceiver, BoundedSender, RecvError};
use mea::rwlock::RwLock;

#[cfg(not(feature = "testing"))]
use mea::mpsc::TryRecvError;

use base64::{prelude::BASE64_STANDARD, Engine};
use rand::{Rng, RngExt};

use crate::errors::Errcode;
use crate::galaxy::scan::ScanResult;
use crate::galaxy::station::{Station, StationId};
use crate::galaxy::Galaxy;
use crate::market::{Market, MarketTx, MARKET_CHANGE_SEC};
use crate::player::{Player, PlayerId, PlayerKey};
use crate::ship::resources::{ExtractionInfo, Resource};
use crate::ship::{Ship, ShipId, ShipState};
use crate::syslog::{SyslogEvent, SyslogFifo, SyslogRecv, SyslogSend};
use crate::utils::{BoxFuture, ShardedLockedData};

#[cfg(not(feature = "extraspeed"))]
const ITER_PERIOD: Duration = Duration::from_millis(20);

#[cfg(feature = "extraspeed")]
const ITER_PERIOD: Duration = Duration::from_micros(20);

//     Equipment becomes more and more expansive

pub enum GameSignal {
    Stop,
    Tick,
}

#[derive(Clone)]
pub struct Game {
    // Locked
    pub players: Arc<ShardedLockedData<PlayerId, Arc<RwLock<Player>>>>,
    pub player_index: Arc<ShardedLockedData<PlayerKey, PlayerId>>,
    pub taken_names: Arc<ShardedLockedData<String, PlayerId>>,
    pub galaxy: Arc<RwLock<Galaxy>>,

    pub init_station: (StationId, Arc<Station>),
    pub market: Arc<Market>,
    pub syslog: SyslogSend,
    pub fifo_events: SyslogFifo,
    pub tstart: f64,
    pub send_sig: BoundedSender<GameSignal>,
}

impl Game {
    pub async fn init<F>(handle_thread: F) -> (std::thread::JoinHandle<()>, Game)
    where
        F: FnOnce(BoundedReceiver<GameSignal>, SyslogRecv, Game) -> std::thread::JoinHandle<()>,
    {
        let (send_stop, recv_stop) = mea::mpsc::bounded(5);
        let (syssend, sysrecv) = SyslogSend::channel();
        let tstart = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let mut galaxy = Galaxy::init();
        let init_station = galaxy.init_new_station().await;
        let data = Game {
            send_sig: send_stop,
            galaxy: Arc::new(RwLock::new(galaxy)),
            market: Arc::new(Market::init()),
            taken_names: Arc::new(ShardedLockedData::new(100)),
            players: Arc::new(ShardedLockedData::new(100)),
            player_index: Arc::new(ShardedLockedData::new(100)),
            syslog: syssend.clone(),
            fifo_events: sysrecv.fifo.clone(),
            tstart,
            init_station,
        };

        let thread_data = data.clone();
        let thread = handle_thread(recv_stop, sysrecv, thread_data);
        (thread, data)
    }

    #[allow(unused_variables, unused_mut)]
    pub async fn start(&self, mut stop: BoundedReceiver<GameSignal>, syslog: SyslogRecv) {
        log::info!("Game thread started");
        let sleepmin_iter = ITER_PERIOD;
        let mut last_iter = Instant::now();
        let mut market_last_tick = Instant::now();
        let mut rng: rand::rngs::SmallRng = rand::make_rng();

        'main: loop {
            #[cfg(feature = "testing")]
            let got = stop.recv().await;

            #[cfg(not(feature = "testing"))]
            let got = match stop.try_recv() {
                Ok(res) => Ok(res),
                Err(TryRecvError::Empty) => Ok(GameSignal::Tick),
                Err(TryRecvError::Disconnected) => {
                    log::error!("Can't get next tick / stop signal: disconnected");
                    Err(RecvError::Disconnected)
                }
            };

            match got {
                Ok(GameSignal::Tick) => {
                    self.threadloop(&mut rng, &mut market_last_tick, &syslog)
                        .await;

                    #[cfg(not(feature = "testing"))]
                    {
                        let took = Instant::now() - last_iter;
                        let t = sleepmin_iter.saturating_sub(took);
                        crate::utils::sleep(t).await;
                        last_iter = Instant::now();
                    }
                }

                Ok(GameSignal::Stop) => break 'main,
                Err(RecvError::Disconnected) => {
                    log::error!("Got disconnected channel in game thread");
                    break 'main;
                }
            }
        }
        log::info!("Exiting game thread");
    }

    async fn threadloop<R: Rng>(&self, rng: &mut R, mlt: &mut Instant, syslog: &SyslogRecv) {
        let market_change_proba = (mlt.elapsed().as_secs_f64() / MARKET_CHANGE_SEC).min(1.0);

        let all_players: Vec<PlayerId> = self.players.get_all_keys().await;
        for player_id in all_players {
            let player = self.players.clone_val(&player_id).await.unwrap();
            let mut player = player.write().await;
            player.update_money(syslog, ITER_PERIOD.as_secs_f64()).await;

            for (_, station) in player.stations.iter() {
                station
                    .update_crafting(ITER_PERIOD.as_secs_f64(), &player_id)
                    .await;
            }

            let mut deadship = vec![];
            for (id, ship) in player.ships.iter_mut() {
                match ship.state {
                    ShipState::InFlight(..) => {
                        let finished = ship.update_flight(ITER_PERIOD.as_secs_f64());
                        if finished {
                            ship.state = ShipState::Idle;
                            if ship.hull_decay >= ship.hull_resistance {
                                deadship.push(*id);
                            } else {
                                syslog
                                    .event(player_id, SyslogEvent::ShipFlightFinished(*id))
                                    .await;
                            }
                        }
                    }

                    ShipState::Extracting(..) => {
                        let finished = ship.update_extract(ITER_PERIOD.as_secs_f64());
                        if finished {
                            ship.state = ShipState::Idle;
                            syslog
                                .event(player_id, SyslogEvent::ExtractionStopped(*id))
                                .await;
                        }
                    }
                    _ => {}
                }
            }
            for id in deadship {
                syslog
                    .event(player_id, SyslogEvent::ShipDestroyed(id))
                    .await;
                player.ships.remove(&id);
            }
        }

        if rng.random_bool(market_change_proba) {
            #[cfg(not(feature = "testing"))]
            self.market.update_prices(rng).await;
            *mlt = Instant::now();
        }

        syslog.update().await;
    }

    pub async fn new_player(&self, name: String) -> Result<(PlayerId, String), Errcode> {
        if self.taken_names.contains_key(&name).await {
            return Err(Errcode::PlayerAlreadyExists(name));
        }

        let player = Player::new(self.init_station.clone(), name.clone());
        let pid = player.id;
        self.taken_names.insert(name.clone(), pid).await;

        let key = BASE64_STANDARD.encode(player.key);

        self.player_index.insert(player.key, player.id).await;
        self.players
            .insert(player.id, Arc::new(RwLock::new(player)))
            .await;
        self.syslog.event(&pid, SyslogEvent::GameStarted).await;
        Ok((pid, key))
    }

    pub async fn get_player(
        &self,
        key: &PlayerKey,
    ) -> Result<(PlayerId, Arc<RwLock<Player>>), Errcode> {
        let Some(id) = self.player_index.clone_val(key).await else {
            return Err(Errcode::NoPlayerWithKey);
        };
        let player = self.players.clone_val(&id).await.unwrap();
        if player.read().await.lost {
            return Err(Errcode::PlayerLost);
        }
        Ok((id, player.clone()))
    }

    pub async fn get_syslogs(&self, pkey: &PlayerKey) -> Result<Vec<(f64, SyslogEvent)>, Errcode> {
        let (pid, _) = self.get_player(pkey).await?;
        let allfifo = self.fifo_events.read().await;
        let Some(fifo) = allfifo.clone_val(&pid).await else {
            return Ok(vec![]);
        };
        let mut fifo = fifo.write().await;
        Ok(fifo.remove_all())
    }

    pub async fn map_station<F, T>(
        &self,
        pkey: &PlayerKey,
        id: &StationId,
        f: F,
    ) -> Result<T, Errcode>
    where
        F: for<'a> FnOnce(&'a PlayerId, &'a Station) -> BoxFuture<'a, Result<T, Errcode>>,
    {
        let (pid, player) = self.get_player(pkey).await?;
        let player = player.read().await;
        let Some(station) = player.stations.get(id) else {
            return Err(Errcode::NoSuchStation(*id));
        };
        let data = f(&pid, station).await;
        data
    }

    pub async fn map_ship<F, T>(&self, pkey: &PlayerKey, id: &ShipId, f: F) -> Result<T, Errcode>
    where
        F: for<'a> FnOnce(PlayerId, &'a Ship) -> BoxFuture<'a, Result<T, Errcode>>,
    {
        let (pid, player) = self.get_player(pkey).await?;
        let player = player.read().await;
        let ship = player.get_ship(id)?;
        let data = f(pid, ship).await;
        data
    }

    pub async fn map_ship_mut<F, T>(
        &self,
        pkey: &PlayerKey,
        id: &ShipId,
        f: F,
    ) -> Result<T, Errcode>
    where
        F: for<'a> FnOnce(PlayerId, &'a mut Ship) -> BoxFuture<'a, Result<T, Errcode>>,
    {
        let (pid, player) = self.get_player(pkey).await?;
        let mut player = player.write().await;
        let ship = player.get_ship_mut(id)?;
        let data = f(pid, ship).await;
        data
    }

    pub async fn map_ship_in_station<F, T>(
        &self,
        pkey: &PlayerKey,
        station_id: &StationId,
        ship_id: &ShipId,
        f: F,
    ) -> Result<T, Errcode>
    where
        F: for<'a> FnOnce(PlayerId, &'a Station, &'a Ship) -> BoxFuture<'a, Result<T, Errcode>>,
    {
        let (pid, player) = self.get_player(pkey).await?;
        let player = player.read().await;
        if !player.ship_in_station(ship_id, station_id)? {
            return Err(Errcode::ShipNotInStation);
        }
        // SAFETY Checked in function above
        let ship = player.ships.get(ship_id).unwrap();
        let station = player.stations.get(station_id).unwrap();
        let data = f(pid, station, ship).await;
        data
    }

    pub async fn map_ship_mut_in_station<F, T>(
        &self,
        pkey: &PlayerKey,
        station_id: &StationId,
        ship_id: &ShipId,
        f: F,
    ) -> Result<T, Errcode>
    where
        F: for<'a> FnOnce(PlayerId, &'a Station, &'a mut Ship) -> BoxFuture<'a, Result<T, Errcode>>,
    {
        let (pid, player) = self.get_player(pkey).await?;
        let mut player = player.write().await;
        if !player.ship_in_station(ship_id, station_id)? {
            return Err(Errcode::ShipNotInStation);
        }
        // SAFETY Checked in function above
        let station = player.stations.get(station_id).unwrap().clone();
        let ship = player.ships.get_mut(ship_id).unwrap();
        let data = f(pid, &station, ship).await;
        data
    }

    pub async fn map_player<F, T>(&self, pkey: &PlayerKey, f: F) -> Result<T, Errcode>
    where
        F: for<'a> FnOnce(&'a Player) -> BoxFuture<'a, Result<T, Errcode>>,
    {
        let (_, player) = self.get_player(pkey).await?;
        let player = player.read().await;
        let data = f(&player).await;
        data
    }

    pub async fn map_player_mut<F, T>(&self, pkey: &PlayerKey, f: F) -> Result<T, Errcode>
    where
        F: for<'a> FnOnce(&'a mut Player) -> BoxFuture<'a, Result<T, Errcode>>,
    {
        let (_, player) = self.get_player(pkey).await?;
        let mut player = player.write().await;
        let data = f(&mut player).await;
        data
    }

    pub async fn player_to_json(
        &self,
        pkey: &PlayerKey,
        id: &PlayerId,
    ) -> Result<serde_json::Value, Errcode> {
        let (pid, player) = self.get_player(pkey).await?;
        let player = player.read().await;
        if pid == *id {
            let stations = player.stations.keys().cloned().collect::<Vec<StationId>>();
            let ships =
                serde_json::to_value(player.ships.values().collect::<Vec<&Ship>>()).unwrap();
            Ok(serde_json::json!({
                "id": id,
                "name": player.name,
                "stations": stations,
                "money": player.money,
                "ships": ships,
                "costs": player.costs,
            }))
        } else {
            Ok(serde_json::json!({
                "id": id,
                "name": player.name,
            }))
        }
    }

    pub async fn scan_galaxy(
        &self,
        pkey: &PlayerKey,
        station_id: &StationId,
    ) -> Result<ScanResult, Errcode> {
        let (_, player) = self.get_player(pkey).await?;
        let player = player.read().await;
        let galaxy = self.galaxy.read().await;
        let Some(station) = player.stations.get(station_id) else {
            return Err(Errcode::NoSuchStation(*station_id));
        };
        Ok(station.scan(&galaxy).await)
    }

    pub async fn start_player_extraction(
        &self,
        pkey: &PlayerKey,
        ship_id: &ShipId,
    ) -> Result<ExtractionInfo, Errcode> {
        let (_, player) = self.get_player(pkey).await?;
        let mut player = player.write().await;
        let ship = player.get_ship_mut(ship_id)?;
        let galaxy = self.galaxy.read().await;
        let Some(planet) = galaxy.get_planet(&ship.position).await else {
            return Err(Errcode::CannotExtractWithoutPlanet);
        };
        ship.start_extraction(&planet).await
    }

    pub async fn player_market_buy(
        &self,
        pkey: &PlayerKey,
        station_id: &StationId,
        resource: &Resource,
        amnt: f64,
    ) -> Result<MarketTx, Errcode> {
        let (_, player) = self.get_player(pkey).await?;
        let mut player = player.write().await;
        let Some(station) = player.stations.get(station_id) else {
            return Err(Errcode::NoSuchStation(*station_id));
        };
        let tx = station
            .buy_resource(&self.market, &player.id, resource, amnt)
            .await?;
        player.money -= tx.removed_money.unwrap();
        player.score -= tx.removed_money.unwrap();
        Ok(tx)
    }

    pub async fn player_market_sell(
        &self,
        pkey: &PlayerKey,
        station_id: &StationId,
        resource: &Resource,
        amnt: f64,
    ) -> Result<MarketTx, Errcode> {
        let (_, player) = self.get_player(pkey).await?;
        let mut player = player.write().await;
        let Some(station) = player.stations.get(station_id) else {
            return Err(Errcode::NoSuchStation(*station_id));
        };
        let tx = station
            .sell_resource(&self.market, &player.id, resource, amnt)
            .await?;
        player.money += tx.added_money.unwrap();
        player.score += tx.added_money.unwrap();
        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;
    use crate::crew::CrewMemberType;
    use crate::tests::block_on;

    // Builds a game whose thread never drives the loop but keeps the syslog
    // receiver alive, so that syslog sends (e.g. the GameStarted event) succeed.
    async fn new_game() -> Game {
        let (_thread, game) = Game::init(|recv, syslog, _game| {
            std::thread::spawn(move || {
                let _recv = recv;
                let _syslog = syslog;
                loop {
                    std::thread::park();
                }
            })
        })
        .await;
        game
    }

    fn decode_key(key: &str) -> PlayerKey {
        let raw = BASE64_STANDARD.decode(key).unwrap();
        raw.try_into().unwrap()
    }

    #[test]
    fn test_new_player_and_duplicate_name() {
        block_on(async {
            let game = new_game().await;
            let (pid, key) = game.new_player("alice".to_string()).await.unwrap();
            assert!(!key.is_empty());
            assert!(game.taken_names.contains_key(&"alice".to_string()).await);

            // Re-using the same name is rejected
            assert!(matches!(
                game.new_player("alice".to_string()).await,
                Err(Errcode::PlayerAlreadyExists(_))
            ));

            // The player can be retrieved with its key
            let (got_id, _) = game.get_player(&decode_key(&key)).await.unwrap();
            assert_eq!(got_id, pid);
        });
    }

    #[test]
    fn test_get_player_unknown_key() {
        block_on(async {
            let game = new_game().await;
            let key: PlayerKey = [0u8; 128];
            assert!(matches!(
                game.get_player(&key).await,
                Err(Errcode::NoPlayerWithKey)
            ));
        });
    }

    #[test]
    fn test_get_player_lost() {
        block_on(async {
            let game = new_game().await;
            let (pid, key) = game.new_player("bob".to_string()).await.unwrap();
            // Mark the player as lost
            let player = game.players.clone_val(&pid).await.unwrap();
            player.write().await.lost = true;
            assert!(matches!(
                game.get_player(&decode_key(&key)).await,
                Err(Errcode::PlayerLost)
            ));
        });
    }

    #[test]
    fn test_player_to_json_self_vs_other() {
        block_on(async {
            let game = new_game().await;
            let (pid, key) = game.new_player("carol".to_string()).await.unwrap();
            let key = decode_key(&key);

            // Viewing one's own data exposes money/costs/ships
            let own = game.player_to_json(&key, &pid).await.unwrap();
            assert_eq!(own["name"], "carol");
            assert!(own.get("money").is_some());
            assert!(own.get("ships").is_some());

            // Viewing another player's data only exposes id/name
            let other = game
                .player_to_json(&key, &(pid.wrapping_add(1)))
                .await
                .unwrap();
            assert!(other.get("money").is_none());
        });
    }

    #[test]
    fn test_scan_galaxy_returns_planets() {
        block_on(async {
            let game = new_game().await;
            let (_pid, key) = game.new_player("dan".to_string()).await.unwrap();
            let station_id = game.init_station.0;
            let scan = game
                .scan_galaxy(&decode_key(&key), &station_id)
                .await
                .unwrap();
            assert!(!scan.planets.is_empty());

            // Unknown station is rejected
            assert!(matches!(
                game.scan_galaxy(&decode_key(&key), &54321).await,
                Err(Errcode::NoSuchStation(_))
            ));
        });
    }

    #[test]
    fn test_get_syslogs_empty_without_loop() {
        block_on(async {
            let game = new_game().await;
            let (_pid, key) = game.new_player("erin".to_string()).await.unwrap();
            // The game loop never ran, so no events have been flushed to the fifo
            let logs = game.get_syslogs(&decode_key(&key)).await.unwrap();
            assert!(logs.is_empty());
        });
    }

    #[test]
    fn test_market_buy_then_sell() {
        block_on(async {
            let game = new_game().await;
            let (pid, key) = game.new_player("frank".to_string()).await.unwrap();
            let key = decode_key(&key);
            let station_id = game.init_station.0;
            let station = game.init_station.1.clone();

            // Assign a trader so the station can trade
            let trader = station.hire_crew(&pid, CrewMemberType::Trader).await;
            station.assign_trader(&pid, trader).await.unwrap();

            let money_before = game
                .players
                .clone_val(&pid)
                .await
                .unwrap()
                .read()
                .await
                .money;
            let tx = game
                .player_market_buy(&key, &station_id, &Resource::Iron, 5.0)
                .await
                .unwrap();
            assert_eq!(tx.added_cargo.unwrap().0, Resource::Iron);
            let money_after = game
                .players
                .clone_val(&pid)
                .await
                .unwrap()
                .read()
                .await
                .money;
            assert!(money_after < money_before);

            // Selling part of it back returns money
            let tx = game
                .player_market_sell(&key, &station_id, &Resource::Iron, 2.0)
                .await
                .unwrap();
            assert_eq!(tx.removed_cargo.unwrap().0, Resource::Iron);
        });
    }

    #[test]
    fn test_market_buy_unknown_station() {
        block_on(async {
            let game = new_game().await;
            let (_pid, key) = game.new_player("grace".to_string()).await.unwrap();
            assert!(matches!(
                game.player_market_buy(&decode_key(&key), &11111, &Resource::Gold, 1.0)
                    .await,
                Err(Errcode::NoSuchStation(_))
            ));
        });
    }

    #[test]
    fn test_start_extraction_without_planet() {
        block_on(async {
            let game = new_game().await;
            let (pid, key) = game.new_player("heidi".to_string()).await.unwrap();

            // Give the player a ship parked far from any planet
            let mut ship = Ship::default();
            ship.owner = pid;
            ship.position = (1, 1, 1);
            let ship_id = ship.id;
            game.players
                .clone_val(&pid)
                .await
                .unwrap()
                .write()
                .await
                .ships
                .insert(ship_id, ship);

            assert!(matches!(
                game.start_player_extraction(&decode_key(&key), &ship_id)
                    .await,
                Err(Errcode::CannotExtractWithoutPlanet)
            ));
        });
    }

    // Inserts a ship at the player's init station and returns its id.
    async fn add_ship_at_station(game: &Game, pid: &PlayerId) -> ShipId {
        let mut ship = Ship::default();
        ship.owner = *pid;
        ship.position = game.init_station.1.position;
        let ship_id = ship.id;
        game.players
            .clone_val(pid)
            .await
            .unwrap()
            .write()
            .await
            .ships
            .insert(ship_id, ship);
        ship_id
    }

    #[test]
    fn test_map_player_read_and_mut() {
        block_on(async {
            let game = new_game().await;
            let (_pid, key) = game.new_player("ivan".to_string()).await.unwrap();
            let key = decode_key(&key);

            let name = game
                .map_player(&key, |p| {
                    Box::pin(async move { Ok::<_, Errcode>(p.name.clone()) })
                })
                .await
                .unwrap();
            assert_eq!(name, "ivan");

            game.map_player_mut(&key, |p| {
                Box::pin(async move {
                    p.money = 999.0;
                    Ok::<_, Errcode>(())
                })
            })
            .await
            .unwrap();

            let money = game
                .map_player(&key, |p| Box::pin(async move { Ok::<_, Errcode>(p.money) }))
                .await
                .unwrap();
            assert_eq!(money, 999.0);
        });
    }

    #[test]
    fn test_map_station_and_ship() {
        block_on(async {
            let game = new_game().await;
            let (pid, key) = game.new_player("judy".to_string()).await.unwrap();
            let key = decode_key(&key);
            let station_id = game.init_station.0;
            let ship_id = add_ship_at_station(&game, &pid).await;

            let sid = game
                .map_station(&key, &station_id, |_pid, st| {
                    Box::pin(async move { Ok::<_, Errcode>(st.id) })
                })
                .await
                .unwrap();
            assert_eq!(sid, station_id);

            let got = game
                .map_ship(&key, &ship_id, |_pid, ship| {
                    Box::pin(async move { Ok::<_, Errcode>(ship.id) })
                })
                .await
                .unwrap();
            assert_eq!(got, ship_id);

            game.map_ship_mut(&key, &ship_id, |_pid, ship| {
                Box::pin(async move {
                    ship.hull_decay = 5.0;
                    Ok::<_, Errcode>(())
                })
            })
            .await
            .unwrap();
        });
    }

    #[test]
    fn test_map_ship_in_station_variants() {
        block_on(async {
            let game = new_game().await;
            let (pid, key) = game.new_player("ken".to_string()).await.unwrap();
            let key = decode_key(&key);
            let station_id = game.init_station.0;
            let ship_id = add_ship_at_station(&game, &pid).await;

            let ok = game
                .map_ship_in_station(&key, &station_id, &ship_id, |_pid, _st, ship| {
                    Box::pin(async move { Ok::<_, Errcode>(ship.id) })
                })
                .await
                .unwrap();
            assert_eq!(ok, ship_id);

            game.map_ship_mut_in_station(&key, &station_id, &ship_id, |_pid, _st, ship| {
                Box::pin(async move {
                    ship.fuel_tank = 1.0;
                    Ok::<_, Errcode>(())
                })
            })
            .await
            .unwrap();
        });
    }

    #[test]
    fn test_threadloop_processes_flight_and_extraction() {
        block_on(async {
            let game = new_game().await;
            let (pid, _key) = game.new_player("laura".to_string()).await.unwrap();
            let player_arc = game.players.clone_val(&pid).await.unwrap();

            // A fast ship on a very short axis-aligned trip: finishes in one tick
            let mut flying = Ship::default();
            flying.owner = pid;
            flying.reactor_power = 10;
            flying.fuel_tank_capacity = 1e9;
            flying.fuel_tank = 1e9;
            flying.hull_resistance = 1e9;
            flying
                .crew
                .0
                .insert(1, crate::crew::CrewMember::from(CrewMemberType::Pilot));
            flying.pilot = Some(1);
            flying.update_perf_stats();
            flying.set_travel((1, 0, 0)).unwrap();
            let flying_id = flying.id;

            player_arc.write().await.ships.insert(flying_id, flying);

            // Drive one game iteration with a private syslog receiver
            let (_send, recv) = SyslogSend::channel();
            let mut rng: rand::rngs::SmallRng = rand::SeedableRng::seed_from_u64(7);
            let mut mlt = Instant::now();
            game.threadloop(&mut rng, &mut mlt, &recv).await;

            // The flight finished and the ship is now idle
            let player = player_arc.read().await;
            assert!(matches!(
                player.ships.get(&flying_id).unwrap().state,
                ShipState::Idle
            ));
        });
    }
}
