use async_graphql::SimpleObject;

use crate::{base_connection::BaseConnection, order_item::OrderItem};

/// A connection of OrderItems.
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

/// Implementation of conversion from BaseConnection<OrderItem> to OrderItemConnection.
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
