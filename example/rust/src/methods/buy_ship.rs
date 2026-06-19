use crate::sdk::{SimeisSDK, get_id};

pub fn buy_ship(sdk: &SimeisSDK, station_id: u64) -> Result<(serde_json::Value, u64), serde_json::Value> {
    println!("---------- Buying first ship ----------");

    let list_all_ships = sdk.list_shop_ship(station_id)?;
    let ship = list_all_ships.first().unwrap().clone();
    let ship_id = get_id(&ship);

    sdk.buy_ship(station_id, ship_id)?;

    Ok((ship, ship_id))
}