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
    pub order_item_inputs: BTreeSet<OrderItemInput>,
}

#[derive(SimpleObject, InputObject, PartialEq, Eq)]
pub struct OrderItemInput {
    /// UUID of shopping cart item associated with order item.
    pub shopping_cart_item_id: Uuid,
    /// Specifies the quantity of the OrderItem.
    pub quantity: u64,
    /// UUID of shipment method to use with order item.
    pub shipment_method_id: Uuid,
    /// UUIDs of coupons to use with order item.
    pub coupons: HashSet<Uuid>,
}

impl PartialOrd for OrderItemInput {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.shopping_cart_item_id
            .partial_cmp(&other.shopping_cart_item_id)
    }
}

impl Ord for OrderItemInput {
    fn cmp(&self, other: &Self) -> Ordering {
        self.shopping_cart_item_id.cmp(&other.shopping_cart_item_id)
    }
}
