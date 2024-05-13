use async_graphql::SimpleObject;

use super::{super::order_item::OrderItem, base_connection::BaseConnection};

/// A connection of order items.
#[derive(SimpleObject)]
#[graphql(shareable)]
pub struct OrderItemConnection {
    /// The resulting entities.
    pub nodes: Vec<OrderItem>,
    /// Whether this connection has a next page.
    pub has_next_page: bool,
    /// The total amount of items in this connection.
    pub total_count: u64,
}

/// Implementation of conversion from `BaseConnection<OrderItem>` to `OrderItemConnection`.
///
/// Prevents GraphQL naming conflicts.
impl From<BaseConnection<OrderItem>> for OrderItemConnection {
    fn from(value: BaseConnection<OrderItem>) -> Self {
        Self {
            nodes: value.nodes,
            has_next_page: value.has_next_page,
            total_count: value.total_count,
        }
    }
}
