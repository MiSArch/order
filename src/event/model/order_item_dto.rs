use bson::Uuid;
use serde::Serialize;

use crate::graphql::model::order_item::OrderItem;

/// Describes DTO of an order item of an order.
///
/// `product_item` is set to `None` as long as `OrderStatus::Pending`.
/// Must contain a ProductItem when `OrderStatus::Placed` or `OrderStatus::Rejected`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderItemDTO {
    /// Order item UUID.
    pub id: Uuid,
    /// Timestamp when order item was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// UUID of product variant associated with order item.
    pub product_variant_id: Uuid,
    /// UUID of product variant version associated with order item.
    pub product_variant_version_id: Uuid,
    /// UUID of tax rate version associated with order item.
    pub tax_rate_version_id: Uuid,
    /// UUID of shopping cart item associated with order item.
    pub shopping_cart_item_id: Uuid,
    /// Specifies the quantity of the order item.
    pub count: u64,
    /// Total cost of product item, which can also be refunded.
    pub compensatable_amount: u64,
    /// UUID of shipment method of order item.
    pub shipment_method_id: Uuid,
    /// UUIDs of discounts applied to order item.
    pub discount_ids: Vec<Uuid>,
}

impl From<OrderItem> for OrderItemDTO {
    fn from(value: OrderItem) -> Self {
        let discount_ids = value
            .internal_discounts
            .iter()
            .map(|discount| discount._id)
            .collect();
        Self {
            id: value._id,
            created_at: value.created_at.to_chrono(),
            product_variant_id: value.product_variant._id,
            product_variant_version_id: value.product_variant_version._id,
            tax_rate_version_id: value.tax_rate_version._id,
            shopping_cart_item_id: value.shopping_cart_item._id,
            count: value.count,
            compensatable_amount: value.compensatable_amount,
            shipment_method_id: value.shipment_method._id,
            discount_ids,
        }
    }
}
