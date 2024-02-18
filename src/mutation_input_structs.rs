use async_graphql::{InputObject, SimpleObject};
use bson::Uuid;
use std::collections::HashSet;

#[derive(SimpleObject, InputObject)]
pub struct CreateOrderInput {
    /// UUID of user owning the order.
    pub user_id: Uuid,
    /// UUIDs of product variants in order.
    pub product_variant_ids: HashSet<Uuid>,
    /// OrderItems of order.
    pub order_items: HashSet<OrderItemInput>,
}

pub struct OrderItemInput {
    /// UUID of product item associated with order item.
    pub product_item_id: Uuid,
    /// UUID of product variant version associated with order item.
    pub product_variant_version_id: Uuid,
    /// UUID of tax rate version associated with order item.
    pub tax_rate_version_id: Uuid,
    /// UUID of shipment method to use with order item.
    pub shipment_method_id: Uuid,
    /// UUIDs of discounts to use with order item.
    pub discounts: HashSet<Uuid>,
}
