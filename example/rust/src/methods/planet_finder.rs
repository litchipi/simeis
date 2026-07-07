use crate::sdk::{get_position, SimeisSDK};

pub fn find_planet(sdk: &SimeisSDK, station_id: u64) -> Result<(serde_json::Value, (u64, u64, u64)), serde_json::Value> {

    println!("---------- Find planets ----------");

    // On a besoin de savoir quelle planète miner pour équiper notre vaisseau
    let all_planets = sdk.scan_planets(station_id)?;

    let nearest_planet = all_planets.first().unwrap();
    let nearest_planet_pos = get_position(nearest_planet).unwrap();

    println!("Targeting planet {nearest_planet:?}");

    Ok((nearest_planet.clone(), nearest_planet_pos))
}