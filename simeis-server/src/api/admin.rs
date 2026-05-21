use std::time::Duration;

use ntex::router::IntoPattern;
use ntex::web::types::Path;
use ntex::web::{self, scope, HttpRequest, ServiceConfig};
use serde_json::json;
use simeis_data::errors::Errcode;
use simeis_data::galaxy::planet::{PlanetId, PlanetInfo};
use simeis_data::player::PlayerId;
use simeis_data::syslog::SyslogEvent;

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

// @summary Removes a player by his ID
// @returns Nothing
#[web::post("/player/remove/{id}")]
async fn remove_player(
    srv: GameState,
    id: Path<PlayerId>,
    req: HttpRequest,
    tok: AdminToken,
) -> impl web::Responder {
    if got_admin_token!(req, tok) {
        let data = srv.remove_player(&id).await.map(|_| json!({}));
        build_response(data)
    } else {
        build_response(Err(Errcode::NotAnAdmin))
    }
}

// @summary Resets the game completely
// @returns Nothing
#[web::post("/reset")]
async fn reset_game(srv: GameState, req: HttpRequest, tok: AdminToken) -> impl web::Responder {
    if got_admin_token!(req, tok) {
        srv.reset().await;
        build_response(Ok(json!({})))
    } else {
        build_response(Err(Errcode::NotAnAdmin))
    }
}

// @summary Resets the game completely in N seconds
// @returns Nothing
#[web::post("/reset/{n}")]
async fn reset_game_cooldown(
    srv: GameState,
    n: Path<u64>,
    req: HttpRequest,
    tok: AdminToken,
) -> impl web::Responder {
    if got_admin_token!(req, tok) {
        let mut n = *n;
        while n > 10 {
            simeis_data::utils::sleep(Duration::from_secs(10)).await;
            n -= 10;
            srv.notify_all_players(SyslogEvent::ResetIn(n)).await;
        }
        if n > 0 {
            simeis_data::utils::sleep(Duration::from_secs(n)).await;
        }
        srv.reset().await;
        build_response(Ok(json!({})))
    } else {
        build_response(Err(Errcode::NotAnAdmin))
    }
}

pub fn configure<T: IntoPattern>(base: T, srv: &mut ServiceConfig) {
    srv.service(
        scope(base)
            .service(get_planet)
            .service(remove_player)
            .service(reset_game)
            .service(reset_game_cooldown),
    );
}
