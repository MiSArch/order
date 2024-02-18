use async_graphql::SimpleObject;

use crate::{base_connection::BaseConnection, foreign_types::Discount};

/// A connection of Discounts.
#[derive(SimpleObject)]
#[graphql(shareable)]
pub struct DiscountConnection {
    /// The resulting entities.
    pub nodes: Vec<Discount>,
    /// Whether this connection has a next page.
    pub has_next_page: bool,
    /// The total amount of items in this connection.
    pub total_count: u64,
}

/// Implementation of conversion from BaseConnection<Discount> to DiscountConnection.
///
/// Prevents GraphQL naming conflicts.
impl From<BaseConnection<Discount>> for DiscountConnection {
    fn from(value: BaseConnection<Discount>) -> Self {
        Self {
            nodes: value.nodes,
            has_next_page: value.has_next_page,
            total_count: value.total_count,
        }
    }
}
