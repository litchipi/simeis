use ntex::router::IntoPattern;
use ntex::web::types::Path;
use ntex::web::{self, scope, HttpRequest, ServiceConfig};
use simeis_data::errors::Errcode;
use simeis_data::galaxy::planet::{PlanetId, PlanetInfo};

use crate::api::build_response;
use crate::{AdminToken, GameState};

// @summary Get the informations about a planet from its ID
// @returns Planet informations
#[web::get("/planet/{id}")]
async fn get_planet(
    srv: GameState,
    id: Path<PlanetId>,
    req: HttpRequest,
    tok: AdminToken,
) -> impl web::Responder {
    if got_admin_token!(req, tok) {
        let data = srv
            .sync_map_planet_id(&id, |pla| {
                Ok(serde_json::to_value(PlanetInfo::scan(u8::MAX, pla)).unwrap())
            })
            .await;
        build_response(data)
    } else {
        build_response(Err(Errcode::NotAnAdmin))
    }
}

pub fn configure<T: IntoPattern>(base: T, srv: &mut ServiceConfig) {
    srv.service(scope(base).service(get_planet));
}
