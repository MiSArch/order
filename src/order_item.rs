use std::{cmp::Ordering, collections::HashSet};

use async_graphql::{ComplexObject, SimpleObject, Result};
use bson::{DateTime, Uuid};
use serde::{Deserialize, Serialize};

use crate::{discount_connection::DiscountConnection, foreign_types::{Discount, ProductItem, ProductVariantVersion, ShipmentMethod, TaxRateVersion}, mutation_input_structs::OrderItemInput, order_datatypes::CommonOrderInput};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, SimpleObject)]
#[graphql(complex)]
pub struct OrderItem {
    /// OrderItem UUID.
    pub _id: Uuid,
    /// Timestamp when OrderItem was created.
    pub created_at: DateTime,
    /// Product item associated with OrderItem.
    pub product_item: ProductItem,
    /// Product variant version associated with OrderItem.
    pub product_variant_version: ProductVariantVersion,
    /// Tax rate version associated with OrderItem.
    pub tax_rate_version: TaxRateVersion,
    /// Total cost of product item, which can also be refunded.
    pub compensatable_amount: u64,
    /// Shipment method of order item.
    pub shipment_method: ShipmentMethod,
    pub internal_discounts: HashSet<Discount>,
}

impl OrderItem {
    pub fn new(order_item_input: &OrderItemInput, created_at: DateTime) -> Self {
        // TODO: Calculate compensatable amount!
        let internal_discounts = order_item_input.discounts.iter().map(|id| Discount { _id: *id }).collect();
        Self {
            _id: Uuid::new(),
            created_at,
            product_item: ProductItem { _id: order_item_input.product_item_id },
            product_variant_version: ProductVariantVersion { _id: order_item_input.product_variant_version_id },
            tax_rate_version: TaxRateVersion { _id: order_item_input.tax_rate_version_id },
            compensatable_amount: 0,
            shipment_method: ShipmentMethod { _id: order_item_input.shipment_method_id },
            internal_discounts,
        }
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