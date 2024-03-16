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
    /// UUID of address to where the order should be shipped to.
    pub shipment_address_id: Uuid,
    /// UUID of address of invoice.
    pub invoice_address_id: Uuid,
    /// UUID of payment information that the order should be processed with.
    pub payment_information_id: Uuid,
}

#[derive(SimpleObject, InputObject, PartialEq, Eq, Clone)]
pub struct OrderItemInput {
    /// UUID of shopping cart item associated with order item.
    pub shopping_cart_item_id: Uuid,
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
