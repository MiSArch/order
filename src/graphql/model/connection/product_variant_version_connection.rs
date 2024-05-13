use async_graphql::SimpleObject;

use super::{super::foreign_types::ProductVariantVersion, base_connection::BaseConnection};

/// A connection of product variant versions.
#[derive(SimpleObject)]
#[graphql(shareable)]
pub struct ProductVariantVersionConnection {
    /// The resulting entities.
    pub nodes: Vec<ProductVariantVersion>,
    /// Whether this connection has a next page.
    pub has_next_page: bool,
    /// The total amount of items in this connection.
    pub total_count: u64,
}

/// Implementation of conversion from `BaseConnection<ProductVariantVersion>` to `ProductVariantVersionConnection`.
///
/// Prevents GraphQL naming conflicts.
impl From<BaseConnection<ProductVariantVersion>> for ProductVariantVersionConnection {
    fn from(value: BaseConnection<ProductVariantVersion>) -> Self {
        Self {
            nodes: value.nodes,
            has_next_page: value.has_next_page,
            total_count: value.total_count,
        }
    }
}
