#![allow(unexpected_cfgs)]
use std::collections::BTreeMap;
use std::sync::Arc;

use rand::rngs::ThreadRng;
use rand::RngExt;

pub mod planet;
pub mod scan;
pub mod station;

use scan::ScanResult;
use station::StationId;

use crate::galaxy::station::Station;

pub type SpaceUnit = u32;
pub type SpaceCoord = (SpaceUnit, SpaceUnit, SpaceUnit);
type GalaxySector = (
    (SpaceUnit, SpaceUnit),
    (SpaceUnit, SpaceUnit),
    (SpaceUnit, SpaceUnit),
);

const SECTOR_SIZE: (SpaceUnit, SpaceUnit, SpaceUnit) = (5000, 5000, 5000);
const PLANETS_PER_SECTOR: usize = 3;
const STATION_FPLANET_DIST: f64 = 500.0;

#[allow(dead_code)]
#[derive(Debug)]
pub enum SpaceObject {
    BaseStation(StationId, Arc<station::Station>),
    Planet(Arc<planet::Planet>),
}

pub struct Galaxy {
    objects: BTreeMap<SpaceCoord, SpaceObject>,
    discovered: Vec<GalaxySector>,
}

impl Galaxy {
    pub fn init() -> Galaxy {
        Galaxy {
            objects: BTreeMap::new(),
            discovered: vec![],
        }
    }

    // X, Y and Z can be any point from the given sector
    // Returns the index in the "discovered" vector
    pub fn generate_sector(&mut self, coord: &SpaceCoord) -> usize {
        let (x, y, z) = coord;
        let (secx, secy, secz) = compute_sector(*x, *y, *z);
        log::debug!(
            "Generating sector ({}-{}, {}-{}, {}-{})",
            secx.0,
            secx.1,
            secy.0,
            secy.1,
            secz.0,
            secz.1,
        );
        let ind = self.discovered.len();
        self.discovered.push((secx, secy, secz));
        let mut rng = rand::rng();
        for _ in 0..PLANETS_PER_SECTOR {
            let x = rng.random_range(secx.0..secx.1);
            let y = rng.random_range(secy.0..secy.1);
            let z = rng.random_range(secz.0..secz.1);
            let planet = planet::Planet::random((x, y, z), &mut rng);
            if self
                .insert(&(x, y, z), SpaceObject::Planet(Arc::new(planet)))
                .is_none()
            {
                continue;
            }
        }
        ind
    }

    pub fn is_discovered(&self, coord: &SpaceCoord) -> bool {
        let (x, y, z) = coord;
        for ((sx, ex), (sy, ey), (sz, ez)) in self.discovered.iter() {
            if (x < sx) || (x > ex) || (y < sy) || (y > ey) || (z < sz) || (z > ez) {
                continue;
            }
            return true;
        }
        false
    }

    pub fn get<'a>(&'a self, coord: &SpaceCoord) -> Option<&'a SpaceObject> {
        self.objects.get(coord)
    }

    pub fn insert(&mut self, coord: &SpaceCoord, obj: SpaceObject) -> Option<()> {
        if self.objects.contains_key(coord) {
            return None;
        }
        self.objects.insert(*coord, obj);
        Some(())
    }

    fn list_objects_in_sector(&self, sector: &GalaxySector) -> Vec<&SpaceObject> {
        let mut objects = vec![];
        for (coord, obj) in self.objects.iter() {
            let (x, y, z) = coord;
            if (x < &sector.0 .0) || (x > &sector.0 .1) {
                continue;
            }
            if (y < &sector.1 .0) || (y > &sector.1 .1) {
                continue;
            }
            if (z < &sector.2 .0) || (z > &sector.2 .1) {
                continue;
            }
            objects.push(obj);
        }
        objects
    }

    pub async fn get_station(&self, coord: &SpaceCoord) -> Option<Arc<station::Station>> {
        let obj = self.get(coord)?;
        let SpaceObject::BaseStation(_, station) = obj else {
            return None;
        };
        Some(station.clone())
    }

    pub async fn get_planet(&self, coord: &SpaceCoord) -> Option<Arc<planet::Planet>> {
        let obj = self.get(coord)?;
        let SpaceObject::Planet(planet) = obj else {
            return None;
        };
        Some(planet.clone())
    }

    pub async fn init_new_station(&mut self) -> (StationId, Arc<Station>) {
        let mut rng = rand::rng();

        let mut seccoord = (rng.random(), rng.random(), rng.random());
        while self.is_discovered(&seccoord) {
            seccoord = (rng.random(), rng.random(), rng.random());
        }
        let id = rng.random();
        let ind = self.generate_sector(&seccoord);
        let sector = self.discovered.get(ind).unwrap();

        let Some(SpaceObject::Planet(pla)) = self
            .list_objects_in_sector(sector)
            .iter()
            .filter(|obj| matches!(obj, SpaceObject::Planet(_)))
            .nth(0)
        else {
            unreachable!("Planet inside generated sector");
        };

        let mut coord;
        let mut retry_n = 0;
        loop {
            coord = get_rand_coord_near(&pla.position, STATION_FPLANET_DIST, &mut rng);
            while !is_in_sector(&coord, sector) || self.get(&coord).is_some() {
                coord = get_rand_coord_near(&pla.position, STATION_FPLANET_DIST, &mut rng);
            }

            let mut mindist = None;
            for pla in self
                .list_objects_in_sector(sector)
                .iter()
                .filter_map(|obj| {
                    if let SpaceObject::Planet(p) = obj {
                        Some(p)
                    } else {
                        None
                    }
                })
            {
                let dist = get_distance(&pla.position, &coord);
                if let Some(ref mut m) = mindist {
                    if dist < *m {
                        *m = dist;
                    }
                } else {
                    mindist = Some(dist);
                }
            }

            let mindist = mindist.unwrap();
            if (mindist - STATION_FPLANET_DIST).abs() < 1.0 {
                break;
            }
            retry_n += 1;
            log::warn!("{retry_n} {mindist} {STATION_FPLANET_DIST}");
            if retry_n > 10000 {
                panic!("Too many retries");
            }
        }
        let station = Arc::new(station::Station::init(id, coord));
        self.insert(&coord, SpaceObject::BaseStation(id, station.clone()))
            .unwrap();
        (id, station)
    }

    pub async fn scan_sector(&self, rank: u8, center: &SpaceCoord) -> ScanResult {
        let strengh = (rank - 1) as f64;
        let mut results = ScanResult::empty();
        debug_assert!(strengh >= 0.0);
        for sector in sectors_around(center, strengh) {
            for obj in self.list_objects_in_sector(&sector) {
                results.add(rank, obj).await;
            }
        }
        debug_assert!(!results.planets.is_empty()); // We should always have some planets
        results
    }
}

#[inline]
pub fn get_delta(a: &SpaceCoord, b: &SpaceCoord) -> (f64, f64, f64) {
    (
        (b.0 as f64) - (a.0 as f64),
        (b.1 as f64) - (a.1 as f64),
        (b.2 as f64) - (a.2 as f64),
    )
}

#[inline]
pub fn get_distance(a: &SpaceCoord, b: &SpaceCoord) -> f64 {
    let delta = get_delta(a, b);
    (delta.0.powf(2.0) + delta.1.powf(2.0) + delta.2.powf(2.0)).sqrt()
}

#[inline]
pub fn get_direction(a: &SpaceCoord, b: &SpaceCoord) -> (f64, f64, f64) {
    let delta = get_delta(a, b);
    let distance = get_distance(a, b);
    (delta.0 / distance, delta.1 / distance, delta.2 / distance)
}

fn compute_sector(x: SpaceUnit, y: SpaceUnit, z: SpaceUnit) -> GalaxySector {
    let start_x = x - (x % SECTOR_SIZE.0);
    let end_x = start_x.saturating_add(SECTOR_SIZE.0);
    let start_y = y - (y % SECTOR_SIZE.1);
    let end_y = start_y.saturating_add(SECTOR_SIZE.1);
    let start_z = z - (z % SECTOR_SIZE.2);
    let end_z = start_z.saturating_add(SECTOR_SIZE.2);
    ((start_x, end_x), (start_y, end_y), (start_z, end_z))
}

pub fn translation(start: SpaceCoord, direction: (f64, f64, f64), dist: f64) -> SpaceCoord {
    (
        ((start.0 as f64) + (dist * direction.0)) as SpaceUnit,
        ((start.1 as f64) + (dist * direction.1)) as SpaceUnit,
        ((start.2 as f64) + (dist * direction.2)) as SpaceUnit,
    )
}

fn is_in_sector(coord: &SpaceCoord, sector: &GalaxySector) -> bool {
    coord.0 >= sector.0 .0
        && coord.0 < sector.0 .1
        && coord.1 >= sector.1 .0
        && coord.1 < sector.1 .1
        && coord.2 >= sector.2 .0
        && coord.2 < sector.2 .1
}

fn sectors_around(center: &SpaceCoord, radius: f64) -> Vec<GalaxySector> {
    let mut sectors = vec![];
    let centersec = compute_sector(center.0, center.1, center.2);

    let xsecstart = ((centersec.0 .0 as f64) - (radius * (SECTOR_SIZE.0 as f64))) as SpaceUnit;
    let nsector_x = (1.0 + (2.0 * radius * (SECTOR_SIZE.0 as f64))) as SpaceUnit;
    let xsecend = ((centersec.0 .1 as f64) + (radius * (SECTOR_SIZE.0 as f64))) as SpaceUnit;
    debug_assert_eq!(xsecstart + (nsector_x * SECTOR_SIZE.0), xsecend);

    let ysecstart = ((centersec.1 .0 as f64) - (radius * (SECTOR_SIZE.1 as f64))) as SpaceUnit;
    let nsector_y = (1.0 + (2.0 * radius * (SECTOR_SIZE.1 as f64))) as SpaceUnit;
    let ysecend = ((centersec.1 .1 as f64) + (radius * (SECTOR_SIZE.1 as f64))) as SpaceUnit;
    debug_assert_eq!(ysecstart + (nsector_y * SECTOR_SIZE.1), ysecend);

    let zsecstart = ((centersec.2 .0 as f64) - (radius * (SECTOR_SIZE.2 as f64))) as SpaceUnit;
    let nsector_z = (1.0 + (2.0 * radius * (SECTOR_SIZE.2 as f64))) as SpaceUnit;
    let zsecend = ((centersec.2 .1 as f64) + (radius * (SECTOR_SIZE.2 as f64))) as SpaceUnit;
    debug_assert_eq!(zsecstart + (nsector_z * SECTOR_SIZE.2), zsecend);

    for sx in 0..nsector_x {
        for sy in 0..nsector_y {
            for sz in 0..nsector_z {
                sectors.push((
                    (
                        xsecstart + (sx * SECTOR_SIZE.0),
                        xsecstart + ((sx + 1) * SECTOR_SIZE.0),
                    ),
                    (
                        ysecstart + (sy * SECTOR_SIZE.1),
                        ysecstart + ((sy + 1) * SECTOR_SIZE.1),
                    ),
                    (
                        zsecstart + (sz * SECTOR_SIZE.2),
                        zsecstart + ((sz + 1) * SECTOR_SIZE.2),
                    ),
                ))
            }
        }
    }

    sectors
}

fn get_rand_coord_near(obj: &SpaceCoord, dist: f64, rng: &mut ThreadRng) -> SpaceCoord {
    let theta = rng.random_range(0.0..2.0 * std::f64::consts::PI); // azimuthal angle
    let phi = rng.random_range(0.0..std::f64::consts::PI); // polar angle
    let x = (obj.0 as f64) + (dist * phi.sin() * theta.cos());
    let y = (obj.1 as f64) + (dist * phi.sin() * theta.sin());
    let z = (obj.2 as f64) + (dist * phi.cos());
    (x as u32, y as u32, z as u32)
}

#[test]
fn test_compute_sector() {
    let mut rng: rand::rngs::SmallRng = rand::make_rng();
    for _ in 0..10000000 {
        let x = rng.random();
        let y = rng.random();
        let z = rng.random();
        let sec = compute_sector(x, y, z);
        assert!(is_in_sector(&(x, y, z), &sec));
    }
    assert_eq!(
        compute_sector(SECTOR_SIZE.0 - 1, 0, 0),
        ((0, SECTOR_SIZE.0), (0, SECTOR_SIZE.1), (0, SECTOR_SIZE.2))
    );
    assert_eq!(
        compute_sector(0, SECTOR_SIZE.1 - 1, 0),
        ((0, SECTOR_SIZE.0), (0, SECTOR_SIZE.1), (0, SECTOR_SIZE.2))
    );
    assert_eq!(
        compute_sector(0, 0, SECTOR_SIZE.2 - 1),
        ((0, SECTOR_SIZE.0), (0, SECTOR_SIZE.1), (0, SECTOR_SIZE.2))
    );

    assert_eq!(
        compute_sector(SECTOR_SIZE.0, 0, 0),
        (
            (SECTOR_SIZE.0, 2 * SECTOR_SIZE.0),
            (0, SECTOR_SIZE.1),
            (0, SECTOR_SIZE.2)
        )
    );
    assert_eq!(
        compute_sector(0, SECTOR_SIZE.1, 0),
        (
            (0, SECTOR_SIZE.0),
            (SECTOR_SIZE.1, 2 * SECTOR_SIZE.1),
            (0, SECTOR_SIZE.2)
        )
    );
    assert_eq!(
        compute_sector(0, 0, SECTOR_SIZE.2),
        (
            (0, SECTOR_SIZE.0),
            (0, SECTOR_SIZE.1),
            (SECTOR_SIZE.2, 2 * SECTOR_SIZE.2)
        )
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_delta() {
        assert_eq!(get_delta(&(0, 0, 0), &(3, 4, 5)), (3.0, 4.0, 5.0));
        assert_eq!(get_delta(&(10, 10, 10), &(5, 7, 10)), (-5.0, -3.0, 0.0));
    }

    #[test]
    fn test_get_distance_pythagorean() {
        // 3-4-5 in 2D plane (z = 0)
        assert_eq!(get_distance(&(0, 0, 0), &(3, 4, 0)), 5.0);
        assert_eq!(get_distance(&(0, 0, 0), &(0, 0, 0)), 0.0);
    }

    #[test]
    fn test_get_distance_is_symmetric() {
        let a = (12, 5, 9);
        let b = (3, 20, 1);
        assert!((get_distance(&a, &b) - get_distance(&b, &a)).abs() < 1e-9);
    }

    #[test]
    fn test_get_direction_is_unit_vector() {
        let dir = get_direction(&(0, 0, 0), &(3, 4, 0));
        let norm = (dir.0.powi(2) + dir.1.powi(2) + dir.2.powi(2)).sqrt();
        assert!((norm - 1.0).abs() < 1e-9);
        assert!((dir.0 - 0.6).abs() < 1e-9);
        assert!((dir.1 - 0.8).abs() < 1e-9);
    }

    #[test]
    fn test_translation_along_axis() {
        let dir = get_direction(&(0, 0, 0), &(1, 0, 0));
        let pos = translation((0, 0, 0), dir, 100.0);
        assert_eq!(pos, (100, 0, 0));
    }

    #[test]
    fn test_translation_full_distance_reaches_destination() {
        let start = (0, 0, 0);
        let dest = (300, 400, 0);
        let dir = get_direction(&start, &dest);
        let dist = get_distance(&start, &dest);
        let pos = translation(start, dir, dist);
        // Rounding to integer space units may differ by 1
        assert!((pos.0 as i64 - dest.0 as i64).abs() <= 1);
        assert!((pos.1 as i64 - dest.1 as i64).abs() <= 1);
    }

    #[test]
    fn test_galaxy_insert_and_get() {
        let mut galaxy = Galaxy::init();
        let coord = (1, 2, 3);
        let planet = planet::Planet::random(coord, &mut rand::rng());
        assert!(galaxy.get(&coord).is_none());
        assert_eq!(
            galaxy.insert(&coord, SpaceObject::Planet(Arc::new(planet))),
            Some(())
        );
        assert!(galaxy.get(&coord).is_some());
    }

    #[test]
    fn test_galaxy_insert_duplicate_rejected() {
        let mut galaxy = Galaxy::init();
        let coord = (4, 5, 6);
        let p1 = planet::Planet::random(coord, &mut rand::rng());
        let p2 = planet::Planet::random(coord, &mut rand::rng());
        assert_eq!(
            galaxy.insert(&coord, SpaceObject::Planet(Arc::new(p1))),
            Some(())
        );
        assert!(galaxy
            .insert(&coord, SpaceObject::Planet(Arc::new(p2)))
            .is_none());
    }

    #[test]
    fn test_is_discovered_after_generate_sector() {
        let mut galaxy = Galaxy::init();
        let coord = (12345, 6789, 4242);
        assert!(!galaxy.is_discovered(&coord));
        galaxy.generate_sector(&coord);
        assert!(galaxy.is_discovered(&coord));
    }

    #[test]
    fn test_is_in_sector() {
        let sector = ((0, 5000), (0, 5000), (0, 5000));
        assert!(is_in_sector(&(0, 0, 0), &sector));
        assert!(is_in_sector(&(4999, 4999, 4999), &sector));
        assert!(!is_in_sector(&(5000, 0, 0), &sector));
    }

    #[test]
    fn test_init_new_station_and_lookups() {
        crate::tests::block_on(async {
            let mut galaxy = Galaxy::init();
            let (id, station) = galaxy.init_new_station().await;
            assert_eq!(station.id, id);

            // The station is retrievable at its coordinate
            let scoord = station.position;
            assert!(galaxy.get_station(&scoord).await.is_some());
            // No planet sits on the station coordinate
            assert!(galaxy.get_planet(&scoord).await.is_none());

            // The sector around the station contains planets
            let scan = galaxy.scan_sector(1, &scoord).await;
            assert!(!scan.planets.is_empty());

            // Each scanned planet is retrievable, and isn't a station
            let pcoord = scan.planets[0].position;
            assert!(galaxy.get_planet(&pcoord).await.is_some());
            assert!(galaxy.get_station(&pcoord).await.is_none());
        });
    }

    #[test]
    fn test_get_returns_none_for_empty_coord() {
        crate::tests::block_on(async {
            let galaxy = Galaxy::init();
            assert!(galaxy.get_station(&(1, 2, 3)).await.is_none());
            assert!(galaxy.get_planet(&(1, 2, 3)).await.is_none());
        });
    }
}
