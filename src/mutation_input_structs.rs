use async_graphql::{InputObject, SimpleObject};
use bson::Uuid;
use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashSet},
};

#[derive(SimpleObject, InputObject)]
pub struct CreateOrderInput {
    /// UUID of user owning the order.
    pub user_id: Uuid,
    /// OrderItems of order.
    pub order_items: BTreeSet<OrderItemInput>,
}

#[derive(SimpleObject, InputObject, PartialEq, Eq)]
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

impl PartialOrd for OrderItemInput {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.product_item_id.partial_cmp(&other.product_item_id)
    }
}

impl Ord for OrderItemInput {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.product_item_id.cmp(&other.product_item_id)
    }
}
