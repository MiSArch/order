use async_graphql::SimpleObject;

use super::{super::order::Order, base_connection::BaseConnection};

/// A connection of orders.
#[derive(SimpleObject)]
#[graphql(shareable)]
pub struct OrderConnection {
    /// The resulting entities.
    pub nodes: Vec<Order>,
    /// Whether this connection has a next page.
    pub has_next_page: bool,
    /// The total amount of items in this connection.
    pub total_count: u64,
}

/// Implementation of conversion from `BaseConnection<Order>` to `OrderConnection`.
///
/// Prevents GraphQL naming conflicts.
impl From<BaseConnection<Order>> for OrderConnection {
    fn from(value: BaseConnection<Order>) -> Self {
        Self {
            nodes: value.nodes,
            has_next_page: value.has_next_page,
            total_count: value.total_count,
        }
    }
}
