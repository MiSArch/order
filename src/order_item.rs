use std::{cmp::Ordering, collections::HashSet};

use async_graphql::{ComplexObject, Result, SimpleObject};
use bson::{DateTime, Uuid};
use mongodb::Collection;
use serde::{Deserialize, Serialize};

use crate::{
    discount_connection::DiscountConnection,
    foreign_types::{Discount, ProductItem, ProductVariantVersion, ShipmentMethod},
    mutation_input_structs::OrderItemInput,
    order_datatypes::{CommonOrderInput, OrderDirection},
    query::query_object,
};

/// Describes an OrderItem of an Order.
///
/// `product_item` is set to None as long as `OrderStatus::Pending`.
/// Must contain a ProductItem when `OrderStatus::Placed` or `OrderStatus::Rejected`.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, SimpleObject)]
#[graphql(complex)]
pub struct OrderItem {
    /// OrderItem UUID.
    pub _id: Uuid,
    /// Timestamp when OrderItem was created.
    pub created_at: DateTime,
    /// Product item associated with OrderItem.
    pub product_item: Option<ProductItem>,
    /// Product variant version associated with OrderItem.
    pub product_variant_version: ProductVariantVersion,
    /// Specifies the quantity of the OrderItem.
    pub quantity: u64,
    /// Total cost of product item, which can also be refunded.
    pub compensatable_amount: u64,
    /// Shipment method of order item.
    pub shipment_method: ShipmentMethod,
    pub internal_discounts: HashSet<Discount>,
}

impl OrderItem {
    /// Constructor for OrderItems.
    ///
    /// Queries ProductVariantVersion from MongoDB.
    /// Correctness: ProductVariantVersion exists, as its precondition is the successful OrderItemInput validation. -> `unwrap()` is uncritical.
    pub async fn try_new(
        order_item_input: &OrderItemInput,
        collection: &Collection<ProductVariantVersion>,
        created_at: DateTime,
    ) -> Result<Self> {
        check_product_item_availability(order_item_input.product_variant_version_id).await?;
        let internal_discounts = query_discounts(&order_item_input.coupons).await?;
        let product_variant_version =
            query_object(&collection, order_item_input.product_variant_version_id)
                .await
                .unwrap();
        let shipment_fee = query_shipment_fee(
            order_item_input.product_variant_version_id,
            order_item_input.quantity,
        );
        let compensatable_amount = calculate_compensatable_amount(
            product_variant_version,
            &internal_discounts,
            shipment_fee,
        );
        Ok(Self {
            _id: Uuid::new(),
            created_at,
            product_item: None,
            product_variant_version,
            quantity: order_item_input.quantity,
            compensatable_amount,
            shipment_method: ShipmentMethod {
                _id: order_item_input.shipment_method_id,
            },
            internal_discounts,
        })
    }
}

#[ComplexObject]
impl OrderItem {
    /// Retrieves discounts.
    async fn discounts(
        &self,
        #[graphql(desc = "Describes that the `first` N discounts should be retrieved.")]
        first: Option<usize>,
        #[graphql(
            desc = "Describes how many discounts should be skipped at the beginning."
        )]
        skip: Option<usize>,
        #[graphql(desc = "Specifies the order in which discounts are retrieved.")] order_by: Option<
            CommonOrderInput,
        >,
    ) -> Result<DiscountConnection> {
        let mut discounts: Vec<Discount> = self.internal_discounts.clone().into_iter().collect();
        sort_discounts(&mut discounts, order_by);
        let total_count = discounts.len();
        let definitely_skip = skip.unwrap_or(0);
        let definitely_first = first.unwrap_or(usize::MAX);
        let discounts_part: Vec<Discount> = discounts
            .into_iter()
            .skip(definitely_skip)
            .take(definitely_first)
            .collect();
        let has_next_page = total_count > discounts_part.len() + definitely_skip;
        Ok(DiscountConnection {
            nodes: discounts_part,
            has_next_page,
            total_count: total_count as u64,
        })
    }
}

impl PartialOrd for OrderItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl Ord for OrderItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self._id.cmp(&other._id)
    }
}

/// Sorts vector of discounts according to BaseOrder.
///
/// * `discounts` - Vector of discounts to sort.
/// * `order_by` - Specifies order of sorted result.
fn sort_discounts(discounts: &mut Vec<Discount>, order_by: Option<CommonOrderInput>) {
    let comparator: fn(&Discount, &Discount) -> bool =
        match order_by.unwrap_or_default().direction.unwrap_or_default() {
            OrderDirection::Asc => |x, y| x < y,
            OrderDirection::Desc => |x, y| x > y,
        };
    discounts.sort_by(|x, y| match comparator(x, y) {
        true => Ordering::Less,
        false => Ordering::Greater,
    });
}

/// Queries inventory service availability of ProductVariantVersion.
async fn check_product_item_availability(product_variant_version_id: Uuid) -> Result<()> {
    todo!();
}

/// Queries Discounts for Coupons from discount service.
async fn query_discounts(coupons: &HashSet<Uuid>) -> Result<HashSet<Discount>> {
    todo!()
}

/// Queries the shipment fee for a ProductVariantVersion of a specific quantity.
fn query_shipment_fee(product_variant_version_id: Uuid, quantity: u64) -> u64 {
    todo!()
}

/// Applies fees and discounts to calculate the compensatable amount of an OrderItem.
fn calculate_compensatable_amount(
    product_variant_version: ProductVariantVersion,
    internal_discounts: &HashSet<Discount>,
    shipment_fee: u64,
) -> u64 {
    todo!()
}
