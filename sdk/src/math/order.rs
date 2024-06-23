use drift::{
    controller::position::PositionDirection,
    state::{
        oracle::OraclePriceData,
        user::{Order, OrderStatus, OrderTriggerCondition, OrderType},
    },
};

use crate::math::auction::{get_auction_price, is_auction_complete};

pub fn get_limit_price(
    order: &Order,
    oracle_price_data: &OraclePriceData,
    slot: u64,
    fallback_price: Option<u64>,
) -> u64 {
    if has_auction_price(order, slot) {
        get_auction_price(order, slot, oracle_price_data.price)
            .try_into()
            .unwrap()
    } else if order.oracle_price_offset != 0 {
        (oracle_price_data.price as i128 + order.oracle_price_offset as i128)
            .try_into()
            .unwrap()
    } else if order.price == 0 {
        match fallback_price {
            Some(price) => price,
            None => {
                dbg!(order);
                panic!("Order price is 0 and no fallback price provided");
            }
        }
    } else {
        order.price
    }
}

fn has_auction_price(order: &Order, slot: u64) -> bool {
    !is_auction_complete(order, slot)
        && (order.auction_start_price != 0 || order.auction_end_price != 0)
}

pub fn is_order_expired(
    order: &Order,
    ts: i64,
    enforce_buffer: Option<bool>,
    buffer_seconds: Option<i64>,
) -> bool {
    let enforce_buffer = enforce_buffer.unwrap_or(false);
    let buffer_seconds = buffer_seconds.unwrap_or(15);

    if must_be_triggered(order) || OrderStatus::Open != order.status || order.max_ts == 0 {
        return false;
    }

    let max_ts = if enforce_buffer && order.is_limit_order() {
        order.max_ts + buffer_seconds
    } else {
        order.max_ts
    };

    ts > max_ts
}

pub fn must_be_triggered(order: &Order) -> bool {
    matches!(
        order.order_type,
        OrderType::TriggerMarket | OrderType::TriggerLimit
    )
}

pub fn is_triggered(order: &Order) -> bool {
    matches!(
        order.trigger_condition,
        OrderTriggerCondition::TriggeredAbove | OrderTriggerCondition::TriggeredBelow
    )
}

pub fn is_resting_limit_order(order: &Order, slot: u64) -> bool {
    if !order.is_limit_order() {
        return false;
    }

    if order.order_type == OrderType::TriggerLimit {
        return match order.direction {
            PositionDirection::Long if order.trigger_price < order.price => {
                return false;
            }
            PositionDirection::Short if order.trigger_price > order.price => {
                return false;
            }
            _ => is_auction_complete(order, slot),
        };
    };

    order.post_only || is_auction_complete(order, slot)
}
