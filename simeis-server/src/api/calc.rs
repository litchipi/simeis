// TODO (#35) Requires POST body implementation first
// TODO (#36) Endpoints for all kinds of computations
// - Mining rate (+ wages cost) of a module, with a certain rank, with a certain operator rank, on a certain planet
// - Traveling costs to a particular destination, from a particular source
// - Prices of the upgrade of a module + its operator (from a source level to dest level), returns benefits, price, wages, etc...
// - Prices of the upgrade of an industry + its operator (from source lvl to dst lvl), returns benefits, price, wages, etc...
// - Resource requirements / production of an industry at a certain level
// - Price, wage, benefits on ships stats when upgrading a pilot
// - Price, wage, fee reduction when upgrading a trader

use ntex::router::IntoPattern;
use ntex::web::types::Json;
use ntex::web::{self, scope, ServiceConfig};

use crate::api::build_response;
use serde::{Deserialize, Serialize};
use serde_json::json;
use simeis_data::galaxy::SpaceCoord;
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

pub fn configure<T: IntoPattern>(base: T, srv: &mut ServiceConfig) {
    srv.service(scope(base).service(compute_travel_costs));
}
