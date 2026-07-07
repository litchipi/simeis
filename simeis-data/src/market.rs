use mea::rwlock::RwLock;
use rand::{Rng, RngExt};
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use strum::IntoEnumIterator;

use crate::{crew::CrewMember, ship::resources::Resource};

const MAX_AVG_AMPL: f64 = 5.5 / 100.0;
const STD_DIV: f64 = 1.15;
pub const MARKET_CHANGE_SEC: f64 = 20.0;
pub const BASE_FEE_RATE: f64 = 25.0 / 100.0;
const FEE_RATE_DEC_POWF: f64 = 1.15;
const UPD_PRICE_PROBA: f64 = 0.80;

// Buying 500000 worth of a resource will increase the price between 10% and 30%
// After  500000 credits, will be capped at btwn 10 and 30%
const PRICE_INC_CAP: f64 = 500_000.0;
const PRICE_INC_RANGE_MAX: f64 = 70.0 / 100.0;
const PRICE_INC_RANGE_MIN: f64 = 30.0 / 100.0;

#[inline]
pub fn fee_rate(rank: u8) -> f64 {
    BASE_FEE_RATE / (rank as f64).powf(FEE_RATE_DEC_POWF)
}

pub struct Market {
    pub prices: BTreeMap<Resource, RwLock<f64>>,
}

impl Market {
    pub fn init() -> Market {
        let mut prices = BTreeMap::new();
        for r in Resource::iter() {
            prices.insert(r, RwLock::new(r.base_price()));
        }
        Market { prices }
    }

    pub async fn to_json(&self) -> serde_json::Value {
        let mut resources = BTreeMap::new();
        for (res, price) in self.prices.iter() {
            let price = price.read().await;
            resources.insert(res, *price);
        }
        serde_json::to_value(resources).unwrap()
    }

    fn rand_distrib(&self, r: &Resource, now_price: f64) -> Normal<f64> {
        let base_price = r.base_price();
        let pratio = now_price / base_price;
        // 0.3    AVG = 1 - 0.3 = 0.7  * MAX AMPL = 3.5 * 0.7  =  2.45
        // 1.3    AVG = 1 - 1.3 = -0.3 * MAX AMPL = 3.5 * -0.3 = -1.05
        let avg = (1.0 - pratio) * MAX_AVG_AMPL;
        let std = avg.abs() + (MAX_AVG_AMPL / STD_DIV);

        rand_distr::Normal::new(avg, std).unwrap()
    }

    fn get_new_price<R: Rng>(&self, rng: &mut R, r: &Resource, old: f64) -> f64 {
        let distr = self.rand_distrib(r, old);
        let change = distr.sample(rng);
        old * (1.0 + change)
    }

    pub async fn update_prices<R: Rng>(&self, rng: &mut R) {
        for (res, price) in self.prices.iter() {
            if !rng.random_bool(UPD_PRICE_PROBA) {
                continue;
            }
            let mut price = price.write().await;

            let new_price = self.get_new_price(rng, res, *price);
            log::trace!(
                "{res:?} {new_price} ({:?}%)",
                (new_price / res.base_price()) * 100.0
            );
            *price = new_price;
        }
    }

    pub async fn buy(&self, trader: &CrewMember, r: &Resource, amnt: f64) -> MarketTx {
        assert!(amnt > 0.0);
        let fee_rate = fee_rate(trader.rank);

        let price = self.prices.get(r).unwrap();
        let price = price.read().await;
        assert!(*price > 0.0);
        let cost = amnt * *price;
        let fees = cost * fee_rate;

        // let price_inc_max = (cost / PRICE_INC_CAP).max(1.0) * PRICE_INC_RANGE_MAX;
        // let price_inc_min = (cost / PRICE_INC_CAP).max(1.0) * PRICE_INC_RANGE_MIN;
        // let mut rng = rand::rng();
        // let inc = rng.random_range(price_inc_min..=price_inc_max);
        // *self.prices.get_mut(r).unwrap() *= 1.0 + inc;

        MarketTx {
            added_cargo: Some((*r, amnt)),
            removed_money: Some(cost + fees),
            fees,
            ..Default::default()
        }
    }

    pub async fn sell(&self, trader: &CrewMember, r: &Resource, amnt: f64) -> MarketTx {
        assert!(amnt > 0.0);
        let fee_rate = fee_rate(trader.rank);

        let price = self.prices.get(r).unwrap();
        let price = price.read().await;
        assert!(*price > 0.0);
        let cost = amnt * *price;
        let fees = cost * fee_rate;

        // let price_dec_max = (cost / PRICE_INC_CAP).max(1.0) * PRICE_INC_RANGE_MAX;
        // let price_dec_min = (cost / PRICE_INC_CAP).max(1.0) * PRICE_INC_RANGE_MIN;
        // let mut rng = rand::rng();
        // let dec = rng.random_range(price_dec_min..=price_dec_max);
        // *self.prices.get_mut(r).unwrap() *= 1.0 - dec;

        MarketTx {
            removed_cargo: Some((*r, amnt)),
            added_money: Some(cost - fees),
            fees,
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MarketTx {
    pub added_cargo: Option<(Resource, f64)>,
    pub removed_cargo: Option<(Resource, f64)>,

    pub added_money: Option<f64>,
    pub removed_money: Option<f64>,
    pub fees: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crew::{CrewMember, CrewMemberType};
    use crate::tests::block_on;

    fn trader(rank: u8) -> CrewMember {
        CrewMember {
            member_type: CrewMemberType::Trader,
            rank,
        }
    }

    #[test]
    fn test_fee_rate_decreases_with_rank() {
        assert_eq!(fee_rate(1), BASE_FEE_RATE);
        assert!(fee_rate(2) < fee_rate(1));
        assert!(fee_rate(10) < fee_rate(2));
        assert!(fee_rate(10) > 0.0);
    }

    #[test]
    fn test_market_tx_default_is_empty() {
        let tx = MarketTx::default();
        assert!(tx.added_cargo.is_none());
        assert!(tx.removed_cargo.is_none());
        assert!(tx.added_money.is_none());
        assert!(tx.removed_money.is_none());
        assert_eq!(tx.fees, 0.0);
    }

    #[test]
    fn test_market_init_has_all_resources_at_base_price() {
        let market = Market::init();
        for r in Resource::iter() {
            let price = *block_on(market.prices.get(&r).unwrap().read());
            assert_eq!(price, r.base_price(), "wrong init price for {r:?}");
        }
    }

    #[test]
    fn test_buy_adds_cargo_and_removes_money_with_fees() {
        block_on(async {
            let market = Market::init();
            let tx = market.buy(&trader(1), &Resource::Iron, 10.0).await;
            let (res, amnt) = tx.added_cargo.unwrap();
            assert_eq!(res, Resource::Iron);
            assert_eq!(amnt, 10.0);

            let base_cost = 10.0 * Resource::Iron.base_price();
            assert!((tx.fees - base_cost * fee_rate(1)).abs() < 1e-9);
            // Buyer pays the cost plus the fees
            assert!((tx.removed_money.unwrap() - (base_cost + tx.fees)).abs() < 1e-9);
            assert!(tx.added_money.is_none());
        });
    }

    #[test]
    fn test_sell_removes_cargo_and_adds_money_minus_fees() {
        block_on(async {
            let market = Market::init();
            let tx = market.sell(&trader(1), &Resource::Gold, 4.0).await;
            let (res, amnt) = tx.removed_cargo.unwrap();
            assert_eq!(res, Resource::Gold);
            assert_eq!(amnt, 4.0);

            let base_cost = 4.0 * Resource::Gold.base_price();
            // Seller receives the value minus the fees
            assert!((tx.added_money.unwrap() - (base_cost - tx.fees)).abs() < 1e-9);
            assert!(tx.removed_money.is_none());
        });
    }

    #[test]
    fn test_higher_rank_trader_pays_lower_fees() {
        block_on(async {
            let market = Market::init();
            let low = market.buy(&trader(1), &Resource::Copper, 5.0).await;
            let high = market.buy(&trader(5), &Resource::Copper, 5.0).await;
            assert!(high.fees < low.fees);
        });
    }

    #[test]
    fn test_to_json_lists_all_resources() {
        let market = Market::init();
        let json = block_on(market.to_json());
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), Resource::iter().count());
    }

    #[test]
    fn test_update_prices_keeps_prices_finite_and_positive() {
        use rand::{rngs::SmallRng, SeedableRng};
        block_on(async {
            let market = Market::init();
            let mut rng = SmallRng::seed_from_u64(12345);
            // Several rounds exercise the random distribution / new price paths
            for _ in 0..20 {
                market.update_prices(&mut rng).await;
            }
            for r in Resource::iter() {
                let price = *market.prices.get(&r).unwrap().read().await;
                assert!(price.is_finite(), "non-finite price for {r:?}");
            }
        });
    }
}
