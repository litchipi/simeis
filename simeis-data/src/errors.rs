#[derive(Debug)]
pub enum Errcode {
    NoPlayerKey,
    InvalidPlayerKey,
    PlayerNotFound(crate::player::PlayerId),
    PlayerAlreadyExists(String),
    NoPlayerWithKey,
    ShipNotFound(crate::ship::ShipId),
    NotEnoughMoney(f64, f64),
    InvalidArgument(&'static str),
    ShipNotExtracting,
    ShipNotIdle,
    CrewMemberNotIdle(crate::crew::CrewId),
    CrewNotNeeded,
    CannotPerformTravel,
    NullDistance,
    NoSuchStation(crate::galaxy::station::StationId),
    NoSuchModule(crate::ship::module::ShipModuleId),
    CannotExtractWithoutPlanet,
    CannotExtractWithoutModule,
    ShipNotInStation,
    WrongCrewType(crate::crew::CrewMemberType),
    CargoFull,
    NoTraderAssigned,
    NoPilotAssigned,
    BuyNothing,
    SellNothing,
    NoFuelInCargo,
    NoHullInCargo,
    CrewMemberNotFound(crate::crew::CrewId),
    PlayerLost,
    GameSignalSend,
    NoSuchIndustryUnit,
}

impl Errcode {
    pub fn errmsg(&self) -> String {
        match self {
            Errcode::NoPlayerKey => "No player key provided with the request".to_string(),
            Errcode::InvalidPlayerKey => "The provided player key isn't valid".to_string(),
            Errcode::PlayerNotFound(id) => format!("No player was found with this ID: {id}"),
            Errcode::PlayerAlreadyExists(name) => format!("Player {name} already exists"),
            Errcode::NoPlayerWithKey => "No player with this key exists in this game".to_string(),
            Errcode::ShipNotFound(id) => format!("Ship of id {id} not found"),
            Errcode::NotEnoughMoney(got, need) => {
                format!("Not enough money, need {need}, got {got}")
            }
            Errcode::InvalidArgument(arg) => format!("Argument {arg} has an invalid value"),
            Errcode::CrewMemberNotIdle(id) => format!("Crew member {id} is already occupied"),
            Errcode::CrewNotNeeded => "This crew member is not needed aboard this ship".to_string(),
            Errcode::CannotPerformTravel => {
                "This travel cannot be done with the current state of the ship".to_string()
            }
            Errcode::NullDistance => "You already are on this coordinates".to_string(),
            Errcode::NoSuchStation(id) => format!("You don't own any station of id {id}"),
            Errcode::NoSuchModule(id) => format!("Ship module of id {id} doesn't exist"),
            Errcode::CannotExtractWithoutPlanet => {
                "Cannot extract resources, this ship is not on a planet".to_string()
            }
            Errcode::ShipNotInStation => "This ship is not docked on station".to_string(),
            Errcode::WrongCrewType(ctype) => {
                format!("This module requires a crew member of type {ctype:?}")
            }
            Errcode::CargoFull => "The cargo is full".to_string(),
            Errcode::ShipNotIdle => "The ship is already occupied with a task".to_string(),
            Errcode::ShipNotExtracting => "This ship is not extracting".to_string(),
            Errcode::NoTraderAssigned => "This station doesn't have a trader assigned".to_string(),
            Errcode::BuyNothing => "Either you attempted to BUY 0 units, or you don't have enough space in cargo to hold the resources".to_string(),
            Errcode::SellNothing => "Either you attempted to SELL 0 units, or you don't have any unit of this resource in your cargo".to_string(),
            Errcode::NoFuelInCargo => "You don't have any fuel in the station cargo".to_string(),
            Errcode::NoHullInCargo => "You don't have any hull in the station cargo".to_string(),
            Errcode::CrewMemberNotFound(id) => format!("Crew member of id {id} not found"),
            Errcode::PlayerLost => "This player lost the game and cannot play anymore".to_string(),
            Errcode::NoPilotAssigned => "No pilot is assigned on this ship".to_string(),
            Errcode::GameSignalSend => "Error while sending a game signal to state".to_string(),
            Errcode::CannotExtractWithoutModule => "Cannot extract because the ship doesn't have the required module".to_string(),
            Errcode::NoSuchIndustryUnit => "The provided industry unit doesn't exist".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_error_messages_non_empty() {
        let all = [
            Errcode::NoPlayerKey,
            Errcode::InvalidPlayerKey,
            Errcode::PlayerNotFound(1),
            Errcode::PlayerAlreadyExists("bob".to_string()),
            Errcode::NoPlayerWithKey,
            Errcode::ShipNotFound(2),
            Errcode::NotEnoughMoney(10.0, 20.0),
            Errcode::InvalidArgument("arg"),
            Errcode::ShipNotExtracting,
            Errcode::ShipNotIdle,
            Errcode::CrewMemberNotIdle(3),
            Errcode::CrewNotNeeded,
            Errcode::CannotPerformTravel,
            Errcode::NullDistance,
            Errcode::NoSuchStation(4),
            Errcode::NoSuchModule(5),
            Errcode::CannotExtractWithoutPlanet,
            Errcode::CannotExtractWithoutModule,
            Errcode::ShipNotInStation,
            Errcode::WrongCrewType(crate::crew::CrewMemberType::Pilot),
            Errcode::CargoFull,
            Errcode::NoTraderAssigned,
            Errcode::NoPilotAssigned,
            Errcode::BuyNothing,
            Errcode::SellNothing,
            Errcode::NoFuelInCargo,
            Errcode::NoHullInCargo,
            Errcode::CrewMemberNotFound(6),
            Errcode::PlayerLost,
            Errcode::GameSignalSend,
            Errcode::NoSuchIndustryUnit,
        ];
        for err in all {
            assert!(!err.errmsg().is_empty(), "empty message for {err:?}");
        }
    }

    #[test]
    fn test_error_messages_interpolate_data() {
        assert!(Errcode::PlayerNotFound(42).errmsg().contains("42"));
        assert!(Errcode::ShipNotFound(7).errmsg().contains('7'));
        assert!(Errcode::PlayerAlreadyExists("alice".to_string())
            .errmsg()
            .contains("alice"));
        let msg = Errcode::NotEnoughMoney(10.0, 20.0).errmsg();
        assert!(msg.contains("10") && msg.contains("20"));
        assert!(Errcode::InvalidArgument("speed").errmsg().contains("speed"));
        assert!(Errcode::NoSuchStation(99).errmsg().contains("99"));
    }
}
