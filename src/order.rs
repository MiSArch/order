use std::cmp::Ordering;

use async_graphql::{ComplexObject, Enum, Error, Result, SimpleObject};
use bson::Uuid;
use bson::{datetime::DateTime, Bson};
use serde::{Deserialize, Serialize};

use crate::foreign_types::UserAddress;
use crate::order_datatypes::OrderDirection;
use crate::order_item::OrderItemDTO;
use crate::{
    order_datatypes::CommonOrderInput, order_item::OrderItem,
    order_item_connection::OrderItemConnection, user::User,
};

/// The Order of a user.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, SimpleObject)]
#[graphql(complex)]
pub struct Order {
    /// Order UUID.
    pub _id: Uuid,
    /// User.
    pub user: User,
    /// Timestamp when Order was created.
    pub created_at: DateTime,
    /// The status of the Order.
    pub order_status: OrderStatus,
    /// Timestamp of Order placement. `None` until Order is placed.
    pub placed_at: Option<DateTime>,
    /// The rejection reason if status of the Order is `OrderStatus::Rejected`.
    pub rejection_reason: Option<RejectionReason>,
    /// The internal vector consisting of OrderItems.
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

/// Describes if Order is placed, or yet pending. An Order can be rejected during its lifetime.
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

/// Describes the reason why an Order was rejected, in case of rejection: `OrderStatus::Rejected`.
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

/// DTO of an order of a user.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderDTO {
    /// Order UUID.
    pub id: Uuid,
    /// UUID of user connected with Order.
    pub user_id: Uuid,
    /// Timestamp when Order was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The status of the Order.
    pub order_status: OrderStatus,
    /// Timestamp of Order placement. `None` until Order is placed.
    pub placed_at: chrono::DateTime<chrono::Utc>,
    /// The rejection reason if status of the Order is `OrderStatus::Rejected`.
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
}

impl TryFrom<Order> for OrderDTO {
    type Error = Error;

    fn try_from(value: Order) -> Result<Self, Self::Error> {
        let order_item_dtos = value
            .internal_order_items
            .iter()
            .map(|o| OrderItemDTO::from(o.clone()))
            .collect();
        let message =
            format!("OrderDTO cannot be created, `placed_at` of the given Order is `None`");
        let placed_at = value.placed_at.ok_or(Error::new(message))?.to_chrono();
        let order_dto = Self {
            id: value._id,
            user_id: value.user._id,
            created_at: value.created_at.to_chrono(),
            order_status: value.order_status,
            placed_at,
            rejection_reason: value.rejection_reason,
            order_items: order_item_dtos,
            shipment_address_id: value.shipment_address._id,
            invoice_address_id: value.invoice_address._id,
            compensatable_order_amount: value.compensatable_order_amount,
            payment_information_id: value.payment_information_id,
        };
        Ok(order_dto)
    }
}
