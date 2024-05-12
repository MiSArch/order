use bson::Uuid;
use serde::Serialize;

use crate::event::order_compensation::OrderCompensation;

/// DTO that models an order compensation that is sent as an event and logged in MongoDB.
#[derive(Debug, Serialize)]
pub struct OrderCompensationDTO {
    /// Order compensation UUID.
    pub id: Uuid,
    /// Amount of order compensation.
    pub amount_to_compensate: u64,
}

impl From<OrderCompensation> for OrderCompensationDTO {
    fn from(value: OrderCompensation) -> Self {
        Self {
            id: value._id,
            amount_to_compensate: value.amount_to_compensate,
        }
    }
}
