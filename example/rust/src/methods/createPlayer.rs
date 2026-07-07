use crate::sdk::{SimeisSDK, get_id};
use serde_json::Value;

pub fn create_player(sdk: &SimeisSDK) -> Result<(Value, u64), Value> {
    println!("---------- Create player ----------");

    // to get the player status
    let player = sdk.get_player_status()?;

    // to get the player id
    let player_id = get_id(&player);

    println!("---------- Player created with ID: {} ----------", player_id);

    Ok((player, player_id))
}