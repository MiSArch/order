use std::cmp::Ordering;

use async_graphql::{ComplexObject, Enum, Result, SimpleObject};
use bson::Uuid;
use bson::{datetime::DateTime, Bson};
use serde::{Deserialize, Serialize};

use super::connection::order_item_connection::OrderItemConnection;
use super::foreign_types::UserAddress;
use super::order_datatypes::{CommonOrderInput, OrderDirection};
use super::order_item::OrderItem;
use super::user::User;

/// The order of a user.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, SimpleObject)]
#[graphql(complex)]
pub struct Order {
    /// Order UUID.
    pub _id: Uuid,
    /// User owning order.
    pub user: User,
    /// Timestamp when order was created.
    pub created_at: DateTime,
    /// The status of the order.
    pub order_status: OrderStatus,
    /// Timestamp of order placement. `None` until order is placed.
    pub placed_at: Option<DateTime>,
    /// The rejection reason if status of the order is `OrderStatus::Rejected`.
    pub rejection_reason: Option<RejectionReason>,
    /// The internal vector consisting of order items.
    #[graphql(skip)]
    pub internal_order_items: Vec<OrderItem>,
    /// Address to where the order should be shipped to.
    #[graphql(skip)]
    pub shipment_address: UserAddress,
    /// Address of invoice.
    pub invoice_address: UserAddress,
    /// Total compensatable amount of order.
    pub compensatable_order_amount: u64,
    /// UUID of payment information that the order should be processed with.
    pub payment_information_id: Uuid,
    /// VAT number.
    #[graphql(skip)]
    pub vat_number: String,
}

#[ComplexObject]
impl Order {
    /// Retrieves order items.
    async fn order_items(
        &self,
        #[graphql(desc = "Describes that the `first` N order items should be retrieved.")]
        first: Option<usize>,
        #[graphql(desc = "Describes how many order items should be skipped at the beginning.")]
        skip: Option<usize>,
        #[graphql(desc = "Specifies the order in which order items are retrieved.")]
        order_by: Option<CommonOrderInput>,
    ) -> Result<OrderItemConnection> {
        let mut order_items: Vec<OrderItem> =
            self.internal_order_items.clone().into_iter().collect();
        sort_order_items(&mut order_items, order_by);
        let total_count = order_items.len();
        let definitely_skip = skip.unwrap_or(0);
        let definitely_first = first.unwrap_or(usize::MAX);
        let order_items_part: Vec<OrderItem> = order_items
            .into_iter()
            .skip(definitely_skip)
            .take(definitely_first)
            .collect();
        let has_next_page = total_count > order_items_part.len() + definitely_skip;
        Ok(OrderItemConnection {
            nodes: order_items_part,
            has_next_page,
            total_count: total_count as u64,
        })
    }
}

/// Describes if order is placed, or yet pending. An order can be rejected during its lifetime.
#[derive(Debug, Enum, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    /// Order is saved a a template, this status can only last for max. 1 hour.
    Pending,
    /// Order is placed, which means SAGA for payment, fullfill and other validity checks need to be triggered.
    Placed,
    /// Something went wrong with the order and it was compensated in all relevant serivces.
    Rejected,
}

impl OrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderStatus::Pending => "PENDING",
            OrderStatus::Placed => "PLACED",
            OrderStatus::Rejected => "REJECTED",
        }
    }
}

impl From<OrderStatus> for Bson {
    fn from(value: OrderStatus) -> Self {
        Bson::from(value.as_str())
    }
}

/// Describes the reason why an order was rejected, in case of rejection: `OrderStatus::Rejected`.
#[derive(Debug, Enum, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RejectionReason {
    /// The order was rejected due to its invalid content.
    InvalidOrderData,
    /// The inventory service was not able to reserve inventory items according to the order.
    InventoryReservationFailed,
}

impl From<Order> for Uuid {
    fn from(value: Order) -> Self {
        value._id
    }
}

/// Sorts vector of order items according to BaseOrder.
///
/// * `order_items` - Vector of order items to sort.
/// * `order_by` - Specifies order of sorted result.
fn sort_order_items(order_items: &mut Vec<OrderItem>, order_by: Option<CommonOrderInput>) {
    let comparator: fn(&OrderItem, &OrderItem) -> bool =
        match order_by.unwrap_or_default().direction.unwrap_or_default() {
            OrderDirection::Asc => |x, y| x < y,
            OrderDirection::Desc => |x, y| x > y,
        };
    order_items.sort_by(|x, y| match comparator(x, y) {
        true => Ordering::Less,
        false => Ordering::Greater,
    });
}
