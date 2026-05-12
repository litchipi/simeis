use std::collections::BTreeMap;

use ntex::router::IntoPattern;
use ntex::web::types::Json;
use ntex::web::{self, scope, ServiceConfig};
use simeis_data::errors::Errcode;
use simeis_data::ship::module::{ShipModuleType, calc_extraction_rate};

use crate::GameState;
use crate::api::build_response;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_value};
use simeis_data::galaxy::{SpaceCoord, SpaceObject};
use simeis_data::ship::navigation::Travel;
use simeis_data::ship::shipstats::ShipStats;

#[derive(Serialize, Deserialize)]
pub struct NavigationComputation {
    ship_stats: ShipStats,
    origin: SpaceCoord,
    dest: SpaceCoord,
}

// @summary Compute how much will cost a travel to a specific position (X, Y, Z)
// @returns Travel informations on this destination
#[web::get("/navigation")]
async fn compute_travel_costs(body: Json<NavigationComputation>) -> impl web::Responder {
    let NavigationComputation {
        ship_stats,
        origin,
        dest,
    } = body.0;
    let travel = Travel::new(dest);
    let cost = travel.compute_from_stats(&origin, &ship_stats);
    build_response(cost.map(|v| json!({"costs": v})))
}

#[derive(Serialize, Deserialize)]
pub struct MiningRateComputation {
    module_type: ShipModuleType,
    module_rank: u8,
    operator_rank: u8,
    planet_coord: SpaceCoord,
}

// @summary Compute how much resources can be extracted from a given planet
// @returns The mining rate for each resources
#[web::get("/mining_rate")]
async fn compute_mining_rate(
    srv: GameState,
    body: Json<MiningRateComputation>,
) -> impl web::Responder {
    let MiningRateComputation { module_type, module_rank, operator_rank, planet_coord } = body.0;
    let data = srv.map_galaxy_coord(&planet_coord, |spaceobj| Box::pin(async move {
        let SpaceObject::Planet(planet) = spaceobj else {
            return Err(Errcode::NotAPlanet);
        };
        let mut result = BTreeMap::new();
        for (res, density) in planet.resources_density.iter() {
            if !res.extractable(&module_type, operator_rank) {
                continue;
            }
            let rate = calc_extraction_rate(res, module_rank, operator_rank, *density);
            result.insert(res, rate);
        }
        Ok(to_value(result).unwrap())
    })).await;
    build_response(data)
}

// TODO (#36) Endpoints for all kinds of computations
// - Prices of the upgrade of a module + its operator (from a source level to dest level), returns benefits, price, wages, etc...
// - Prices of the upgrade of an industry + its operator (from source lvl to dst lvl), returns benefits, price, wages, etc...
// - Resource requirements / production of an industry at a certain level
// - Price, wage, benefits on ships stats when upgrading a pilot
// - Price, wage, fee reduction when upgrading a trader

pub fn configure<T: IntoPattern>(base: T, srv: &mut ServiceConfig) {
    srv.service(
        scope(base)
            // TODO (#36) Test these, add to SDK
            .service(compute_travel_costs)
            .service(compute_mining_rate)
    );
}
