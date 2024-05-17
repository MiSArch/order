use async_graphql::InputObject;
use bson::Uuid;
use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashSet},
};

#[derive(Debug, InputObject)]
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
    /// Optional VAT number.
    pub vat_number: Option<String>,
}

#[derive(Debug, InputObject, PartialEq, Eq, Clone)]
pub struct OrderItemInput {
    /// UUID of shopping cart item associated with order item.
    pub shopping_cart_item_id: Uuid,
    /// UUID of shipment method to use with order item.
    pub shipment_method_id: Uuid,
    /// UUIDs of coupons to use with order item.
    pub coupon_ids: HashSet<Uuid>,
}

#[derive(Debug, InputObject, Clone)]
pub struct PaymentAuthorizationInput {
    /// CVC/CVV number of 3-4 digits.
    pub cvc: Option<u16>,
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

#[derive(Debug, InputObject)]
pub struct PlaceOrderInput {
    /// UUID of order to place.
    pub id: Uuid,
    /// Optional payment authorization data.
    pub payment_authorization: Option<PaymentAuthorizationInput>,
}
