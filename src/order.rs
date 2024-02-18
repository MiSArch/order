use std::{cmp::Ordering, collections::HashSet};

use async_graphql::{
    connection::{Edge, EmptyFields}, ComplexObject, Enum, OutputType, Result, SimpleObject
};
use bson::datetime::DateTime;
use bson::Uuid;
use serde::{Deserialize, Serialize};

use crate::{
    foreign_types::ProductVariantVersion, order_datatypes::{CommonOrderInput, OrderDirection}, order_item::OrderItem, order_item_connection::OrderItemConnection, product_variant_version_connection::ProductVariantVersionConnection, user::User
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
    /// The rejection reason if status of the Order is `OrderStatus::Rejected`.
    pub rejection_reason: Option<RejectionReason>,
    pub internal_order_items: HashSet<OrderItem>,
}

#[ComplexObject]
impl Order {
    /// Retrieves order items.
    async fn order_items(
        &self,
        #[graphql(desc = "Describes that the `first` N order items should be retrieved.")]
        first: Option<usize>,
        #[graphql(
            desc = "Describes how many order items should be skipped at the beginning."
        )]
        skip: Option<usize>,
        #[graphql(desc = "Specifies the order in which order items are retrieved.")] order_by: Option<
            CommonOrderInput,
        >,
    ) -> Result<OrderItemConnection> {
        todo!();
        /* let mut product_variants: Vec<ProductVariant> =
            self.internal_product_variants.clone().into_iter().collect();
        sort_product_variants(&mut product_variants, order_by);
        let total_count = product_variants.len();
        let definitely_skip = skip.unwrap_or(0);
        let definitely_first = first.unwrap_or(usize::MAX);
        let product_variants_part: Vec<ProductVariant> = product_variants
            .into_iter()
            .skip(definitely_skip)
            .take(definitely_first)
            .collect();
        let has_next_page = total_count > product_variants_part.len() + definitely_skip;
        Ok(ProductVariantConnection {
            nodes: product_variants_part,
            has_next_page,
            total_count: total_count as u64,
        }) */
    }
}

#[derive(Debug, Enum, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Placed,
    Rejected,
}

#[derive(Debug, Enum, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]

pub enum RejectionReason {
    InvalidOrderData,
    InventoryReservationFailed
}

/// Sorts vector of product variants according to BaseOrder.
///
/// * `product_variants` - Vector of product variants to sort.
/// * `order_by` - Specifies order of sorted result.
fn sort_product_variants(
    product_variants: &mut Vec<ProductVariantVersion>,
    order_by: Option<CommonOrderInput>,
) {
    let comparator: fn(&ProductVariantVersion, &ProductVariantVersion) -> bool =
        match order_by.unwrap_or_default().direction.unwrap_or_default() {
            OrderDirection::Asc => |x, y| x < y,
            OrderDirection::Desc => |x, y| x > y,
        };
    product_variants.sort_by(|x, y| match comparator(x, y) {
        true => Ordering::Less,
        false => Ordering::Greater,
    });
}

impl From<Order> for Uuid {
    fn from(value: Order) -> Self {
        value._id
    }
}

pub struct NodeWrapper<Node>(pub Node);

impl<Node> From<NodeWrapper<Node>> for Edge<uuid::Uuid, Node, EmptyFields>
where
    Node: Into<uuid::Uuid> + OutputType + Clone,
{
    fn from(value: NodeWrapper<Node>) -> Self {
        let uuid = Into::<uuid::Uuid>::into(value.0.clone());
        Edge::new(uuid, value.0)
    }
}
