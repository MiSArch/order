use async_graphql::{OutputType, SimpleObject};
use mongodb_cursor_pagination::FindResult;

/// A base connection for an output type.
#[derive(SimpleObject)]
#[graphql(shareable)]
pub struct BaseConnection<T: OutputType> {
    /// The resulting entities.
    pub nodes: Vec<T>,
    /// Whether this connection has a next page.
    pub has_next_page: bool,
    /// The total amount of items in this connection.
    pub total_count: u64,
}

pub struct FindResultWrapper<Node>(pub FindResult<Node>);

/// Object that writes total count of items in a query, regardless of pagination.
#[derive(SimpleObject)]
pub struct AdditionalFields {
    total_count: u64,
}

/// Implementation of conversion from MongoDB pagination to GraphQL connection.
impl<Node> From<FindResultWrapper<Node>> for BaseConnection<Node>
where
    Node: OutputType,
{
    fn from(value: FindResultWrapper<Node>) -> Self {
        BaseConnection {
            nodes: value.0.items,
            has_next_page: value.0.page_info.has_next_page,
            total_count: value.0.total_count,
        }
    }
}
