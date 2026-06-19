use crate::sdk::{get_id, json_get_bool, SimeisSDK};
use serde_json::Value;

pub fn buy_equipment_based_on_planet(
    sdk: &SimeisSDK,
    station_id: u64,
    ship_id: u64,
    nearest_planet: Value,
) -> Result<u64, Value> {
    println!("---------- Buying equipment based on the planet ----------");

    let planet_is_solid = json_get_bool("solid", &nearest_planet).unwrap();
    let module = if planet_is_solid {
        "Miner"
    } else {
        "GasSucker"
    };
    let module = sdk.buy_module_on_ship(station_id, ship_id, module)?;
    let mod_id = get_id(&module);

    Ok(mod_id)
}