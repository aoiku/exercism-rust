use std::ops::{Add, Div, Mul, Sub};

use drift::{
    math::constants::FIVE_MINUTE,
    state::{
        oracle::{HistoricalOracleData, OraclePriceData},
        perp_market::AMM,
    },
};

pub fn calculate_live_oracle_twap(
    hist_oracle_data: &HistoricalOracleData,
    oracle_price_data: &OraclePriceData,
    now: i128,
    period: i128,
) -> i128 {
    let oracle_twap = if period == FIVE_MINUTE {
        hist_oracle_data.last_oracle_price_twap_5min
    } else {
        hist_oracle_data.last_oracle_price_twap
    };

    let since_last_update =
        std::cmp::max(1, now - hist_oracle_data.last_oracle_price_twap_ts as i128);
    let since_start = std::cmp::max(0, period - since_last_update);

    let clamp_range = oracle_twap / 3;

    let clamped_oracle_price = std::cmp::min(
        oracle_twap + clamp_range,
        std::cmp::max(oracle_price_data.price, oracle_twap - clamp_range),
    );

    let new_oracle_twap = (oracle_twap as i128)
        .mul(since_start)
        .add((clamped_oracle_price as i128).mul(since_last_update))
        .div(since_start.add(since_last_update));

    new_oracle_twap
}

pub fn calculate_live_oracle_std(
    amm: &AMM,
    oracle_price_data: &OraclePriceData,
    now: u128,
) -> u128 {
    let since_last_update = std::cmp::max(
        1,
        now.sub(amm.historical_oracle_data.last_oracle_price_twap_ts as u128),
    );
    // let since_start = std::cmp::max(0, amm.funding_period.sub(since_last_update));

    // let live_oracle_twap = calculate_live_ora
    0
}

#[cfg(test)]
mod tests {
    use drift::{
        math::constants::FIVE_MINUTE,
        state::oracle::{HistoricalOracleData, OraclePriceData},
    };

    use super::calculate_live_oracle_twap;

    #[test]
    fn test_calculate_live_oracle_twap() {
        let hist_data = HistoricalOracleData {
            last_oracle_price_twap: 1000,
            last_oracle_price_twap_5min: 1200,
            last_oracle_price_twap_ts: 100,
            ..Default::default()
        };
        let oracle_data = OraclePriceData {
            price: 1100,
            ..Default::default()
        };

        let now = 200;
        let period = FIVE_MINUTE;

        let result = calculate_live_oracle_twap(&hist_data, &oracle_data, now, period);

        assert_eq!(
            1166, result,
            "The TWAP calculation did not return the expected value"
        );
    }

    #[test]
    fn test_calculate_live_oracle_twap_long_period() {
        let hist_data = HistoricalOracleData {
            last_oracle_price_twap: 1000,
            last_oracle_price_twap_5min: 1200,
            last_oracle_price_twap_ts: 100,
            ..Default::default()
        };
        let oracle_data = OraclePriceData {
            price: 800,
            ..Default::default()
        };

        let now = 200; // Current timestamp
        let period = 1000; // longer period

        let result = calculate_live_oracle_twap(&hist_data, &oracle_data, now, period);

        assert_eq!(
            980, result,
            "The TWAP calculation for longer periods did not return the expected value"
        );
    }
}