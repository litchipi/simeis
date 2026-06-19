use std::ops::Deref;

use serde::{Deserialize, Serialize};

use super::planet::PlanetInfo;
use super::station::StationInfo;
use super::{get_distance, SpaceCoord, SpaceObject};

#[derive(Serialize, Deserialize, Debug)]
pub struct ScanResult {
    pub planets: Vec<PlanetInfo>,
    pub stations: Vec<StationInfo>,
}

impl ScanResult {
    pub fn empty() -> ScanResult {
        ScanResult {
            planets: vec![],
            stations: vec![],
        }
    }

    pub async fn add(&mut self, rank: u8, obj: &SpaceObject) {
        match obj {
            SpaceObject::BaseStation(_, station) => {
                self.stations.push(StationInfo::scan(rank, station.deref()));
            }
            SpaceObject::Planet(planet) => {
                self.planets.push(PlanetInfo::scan(rank, planet.as_ref()))
            }
        }
    }

    pub fn get_closest_planet(&self, pos: &SpaceCoord) -> Option<PlanetInfo> {
        let mut planets = self.planets.clone();
        planets.sort_by(|a, b| {
            let dist_a = get_distance(pos, &a.position);
            let dist_b = get_distance(pos, &b.position);
            dist_a.total_cmp(&dist_b)
        });
        planets.into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planet_at(position: SpaceCoord) -> PlanetInfo {
        PlanetInfo {
            position,
            temperature: 100,
            solid: true,
        }
    }

    #[test]
    fn test_empty_scan_result() {
        let res = ScanResult::empty();
        assert!(res.planets.is_empty());
        assert!(res.stations.is_empty());
    }

    #[test]
    fn test_get_closest_planet_none_when_empty() {
        let res = ScanResult::empty();
        assert!(res.get_closest_planet(&(0, 0, 0)).is_none());
    }

    #[test]
    fn test_get_closest_planet_returns_nearest() {
        let mut res = ScanResult::empty();
        res.planets.push(planet_at((100, 0, 0)));
        res.planets.push(planet_at((10, 0, 0)));
        res.planets.push(planet_at((50, 0, 0)));
        let closest = res.get_closest_planet(&(0, 0, 0)).unwrap();
        assert_eq!(closest.position, (10, 0, 0));
    }
}
