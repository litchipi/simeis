use std::collections::BTreeMap;
use std::time::Instant;

use ntex::web;
use ntex::web::{HttpResponse, ServiceConfig};
use serde_json::json;
use serde_json::Value;

use simeis_data::galaxy::planet::PlanetInfo;
use simeis_data::galaxy::station::StationId;
use simeis_data::galaxy::SpaceObject;
use simeis_data::ship::ShipState;

use crate::api::build_response;
use crate::api::GameState;

// @noswagger
#[web::get("/spectate")]
async fn spectate_ui() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../../../doc/spectate.html"))
}

// @summary Get a live snapshot of the whole game, for spectators
// @returns All players (score, money, wages), their fleets, the known stations,
// the discovered planets & sectors, and the current market prices.
// Meant to be polled by the spectator dashboard on `/spectate`, but any client
// is free to use it (it only exposes what a spectator screen would show anyway).
// Note: `u64` identifiers are serialized as strings, as they exceed the safe
// integer range of JSON numbers in most parsers.
#[web::get("/spectate/data")]
async fn spectate_data(srv: GameState) -> impl web::Responder {
    let mut players = BTreeMap::new();
    let mut ships = vec![];
    let mut stations: BTreeMap<StationId, Value> = BTreeMap::new();

    let all_players = srv.players.get_all_keys().await;
    for pid in all_players {
        let player = srv.players.clone_val(&pid).await.unwrap();
        let player = player.read().await;

        let mut potential = 0.0;
        for (sid, station) in player.stations.iter() {
            potential += station.get_cargo_potential_price(&pid).await;
            stations
                .entry(*sid)
                .or_insert_with(|| json!({ "id": sid, "position": station.position }));
        }

        for ship in player.ships.values() {
            let (state, flight, extraction) = match &ship.state {
                ShipState::Idle => ("idle", Value::Null, Value::Null),
                ShipState::InFlight(data) => (
                    "flight",
                    json!({
                        "start": data.start,
                        "destination": data.destination,
                        "direction": data.direction,
                        "dist_done": data.dist_done,
                        "dist_tot": data.dist_tot,
                    }),
                    Value::Null,
                ),
                ShipState::Extracting(info) => (
                    "extraction",
                    Value::Null,
                    json!(info.mining_rate.keys().collect::<Vec<_>>()),
                ),
            };
            ships.push(json!({
                "id": ship.id.to_string(),
                "owner": pid.to_string(),
                "position": ship.position,
                "speed": ship.stats.speed,
                "fuel": [ship.fuel_tank, ship.fuel_tank_capacity],
                "hull": [ship.hull_decay, ship.hull_resistance],
                "cargo": [ship.cargo.usage, ship.cargo.capacity],
                "state": state,
                "flight": flight,
                "extraction": extraction,
            }));
        }

        let age = (Instant::now() - player.created).as_secs_f64();
        players.insert(
            pid.to_string(),
            json!({
                "name": player.name,
                "score": player.score,
                "potential": potential,
                "money": player.money,
                "costs": player.costs,
                "lost": player.lost,
                "age": age,
                "nships": player.ships.len(),
            }),
        );
    }

    let (planets, sectors) = {
        let galaxy = srv.galaxy.read().await;
        let planets = galaxy
            .iter_objects()
            .filter_map(|(_, obj)| match obj {
                SpaceObject::Planet(planet) => Some(json!(PlanetInfo::scan(1, planet))),
                _ => None,
            })
            .collect::<Vec<Value>>();
        let sectors = json!(galaxy.sectors());
        (planets, sectors)
    };

    let market = srv.market.to_json().await;

    build_response(Ok(json!({
        "tstart": srv.tstart,
        "players": players,
        "ships": ships,
        "stations": stations,
        "planets": planets,
        "sectors": sectors,
        "market": market,
    })))
}

pub fn configure(srv: &mut ServiceConfig) {
    srv.service(spectate_ui).service(spectate_data);
}
