use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::ship::resources::Resource;

use super::SpaceCoord;

// Informations that can be scanned from a planet
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PlanetInfo {
    pub position: SpaceCoord,
    pub temperature: u16,
    pub solid: bool,
}

impl PlanetInfo {
    pub fn scan(_rank: u8, planet: &Planet) -> PlanetInfo {
        PlanetInfo {
            position: planet.position,
            temperature: planet.temperature,
            solid: planet.solid,
        }
    }
}

#[derive(Debug)]
pub struct Planet {
    pub position: SpaceCoord,
    temperature: u16,
    solid: bool,
}

impl Planet {
    pub fn random<R: rand::Rng>(coord: SpaceCoord, rng: &mut R) -> Planet {
        Planet {
            solid: rng.random_bool(0.4),
            temperature: rng.random(),
            position: coord,
        }
    }

    #[allow(clippy::if_same_then_else)]
    pub fn resource_density(&self, resource: &Resource) -> f64 {
        if self.solid && resource.mineable(u8::MAX) {
            6.25
        } else if !self.solid && resource.suckable(u8::MAX) {
            6.25
        } else if resource.suckable(u8::MAX) {
            6.25
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_planet() -> Planet {
        Planet {
            position: (1, 2, 3),
            temperature: 250,
            solid: true,
        }
    }

    fn gas_planet() -> Planet {
        Planet {
            position: (4, 5, 6),
            temperature: 60,
            solid: false,
        }
    }

    #[test]
    fn test_random_planet_keeps_position() {
        let mut rng = rand::rng();
        let planet = Planet::random((10, 20, 30), &mut rng);
        assert_eq!(planet.position, (10, 20, 30));
    }

    #[test]
    fn test_solid_planet_has_mineable_density() {
        let planet = solid_planet();
        assert_eq!(planet.resource_density(&Resource::Iron), 6.25);
        // Solid planets also yield suckable gases per the density rules
        assert_eq!(planet.resource_density(&Resource::Hydrogen), 6.25);
    }

    #[test]
    fn test_gas_planet_has_suckable_density() {
        let planet = gas_planet();
        assert_eq!(planet.resource_density(&Resource::Oxygen), 6.25);
        // Minerals can't be extracted from a gas planet
        assert_eq!(planet.resource_density(&Resource::Iron), 0.0);
    }

    #[test]
    fn test_pumpable_resources_have_no_density() {
        let planet = solid_planet();
        assert_eq!(planet.resource_density(&Resource::Water), 0.0);
        assert_eq!(planet.resource_density(&Resource::Oil), 0.0);
    }

    #[test]
    fn test_planet_info_scan_copies_fields() {
        let planet = solid_planet();
        let info = PlanetInfo::scan(1, &planet);
        assert_eq!(info.position, planet.position);
        assert_eq!(info.temperature, planet.temperature);
        assert_eq!(info.solid, planet.solid);
    }
}
