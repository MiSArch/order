use async_graphql::Error;
use bson::Uuid;
use serde::Serialize;

use crate::graphql::model::{
    order::{Order, OrderStatus, RejectionReason},
    payment_authorization::PaymentAuthorization,
};

use super::order_item_dto::OrderItemDTO;

/// DTO of an order of a user.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderDTO {
    /// Order UUID.
    pub id: Uuid,
    /// UUID of user connected with order.
    pub user_id: Uuid,
    /// Timestamp when order was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The status of the order.
    pub order_status: OrderStatus,
    /// Timestamp of order placement. `None` until order is placed.
    pub placed_at: chrono::DateTime<chrono::Utc>,
    /// The rejection reason if status of the order is `OrderStatus::Rejected`.
    pub rejection_reason: Option<RejectionReason>,
    /// OrderItems associated with the order.
    pub order_items: Vec<OrderItemDTO>,
    /// UUID of address to where the order should be shipped to.
    pub shipment_address_id: Uuid,
    /// UUID of address of invoice.
    pub invoice_address_id: Uuid,
    /// Total compensatable amount of order.
    pub compensatable_order_amount: u64,
    /// UUID of payment information that the order should be processed with.
    pub payment_information_id: Uuid,
    /// Optional payment authorization information.
    pub payment_authorization: Option<PaymentAuthorization>,
    /// VAT number.
    pub vat_number: String,
}

impl TryFrom<(Order, Option<PaymentAuthorization>)> for OrderDTO {
    type Error = Error;

    fn try_from(
        (order, payment_authorization): (Order, Option<PaymentAuthorization>),
    ) -> Result<Self, Self::Error> {
        let order_item_dtos = order
            .internal_order_items
            .iter()
            .map(|order_item| OrderItemDTO::from(order_item.clone()))
            .collect();
        let message =
            format!("OrderDTO cannot be created, `placed_at` of the given Order is `None`");
        let placed_at = order.placed_at.ok_or(Error::new(message))?.to_chrono();
        let order_dto = Self {
            id: order._id,
            user_id: order.user._id,
            created_at: order.created_at.to_chrono(),
            order_status: order.order_status,
            placed_at,
            rejection_reason: order.rejection_reason,
            order_items: order_item_dtos,
            shipment_address_id: order.shipment_address._id,
            invoice_address_id: order.invoice_address._id,
            compensatable_order_amount: order.compensatable_order_amount,
            payment_information_id: order.payment_information_id,
            payment_authorization: payment_authorization,
            vat_number: order.vat_number,
        };
        Ok(order_dto)
    }
}
