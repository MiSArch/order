use std::{cmp::Ordering, collections::BTreeSet};

use async_graphql::{ComplexObject, Result, SimpleObject};
use bson::{DateTime, Uuid};
use serde::{Deserialize, Serialize};

use crate::{
    discount_connection::DiscountConnection,
    foreign_types::{
        Discount, ProductItem, ProductVariantVersion, ShoppingCartItem, TaxRateVersion,
    },
    mutation_input_structs::OrderItemInput,
    order_datatypes::{CommonOrderInput, OrderDirection},
    shipment::{Shipment, ShipmentMethod, ShipmentStatus},
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
    /// Tax rate version associated with OrderItem.
    pub tax_rate_version: TaxRateVersion,
    /// Shopping cart item associated with OrderItem.
    pub shopping_cart_item: ShoppingCartItem,
    /// Specifies the quantity of the OrderItem.
    pub count: u64,
    /// Total cost of product item, which can also be refunded.
    pub compensatable_amount: u64,
    /// Shipment of order item.
    pub shipment: Shipment,
    pub internal_discounts: BTreeSet<Discount>,
}

impl OrderItem {
    /// Constructor for OrderItems.
    ///
    /// Queries ProductVariantVersion from MongoDB.
    /// Correctness: ProductVariantVersion exists, as its precondition is the successful OrderItemInput validation. -> `unwrap()` is uncritical.
    pub fn new(
        order_item_input: &OrderItemInput,
        product_variant_version: ProductVariantVersion,
        tax_rate_version: TaxRateVersion,
        count: u64,
        internal_discounts: BTreeSet<Discount>,
        shipment_fee: u64,
        current_timestamp: DateTime,
    ) -> Self {
        let compensatable_amount = calculate_compensatable_amount(
            product_variant_version,
            &internal_discounts,
            shipment_fee,
        );
        let shopping_cart_item = ShoppingCartItem {
            _id: order_item_input.shopping_cart_item_id,
        };
        let shipment = Shipment {
            _id: Uuid::new(),
            status: ShipmentStatus::Pending,
            shipment_method: ShipmentMethod {
                _id: order_item_input.shipment_method_id,
            },
        };
        Self {
            _id: Uuid::new(),
            created_at: current_timestamp,
            product_item: None,
            product_variant_version,
            tax_rate_version,
            shopping_cart_item,
            count,
            compensatable_amount,
            shipment,
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

/// Applies fees and discounts to calculate the compensatable amount of an OrderItem.
fn calculate_compensatable_amount(
    product_variant_version: ProductVariantVersion,
    internal_discounts: &BTreeSet<Discount>,
    shipment_fee: u64,
) -> u64 {
    let undiscounted_price = product_variant_version.price as f64;
    let maybe_max_discountable_amount = internal_discounts
        .iter()
        .map(|d| d.max_discountable_amount)
        .min()
        .and_then(|v| Some(v as f64));
    let mut discounted_price = internal_discounts
        .iter()
        .fold(undiscounted_price, |prev_price, discount| {
            prev_price * discount.discount
        });
    if let Some(max_discountable_amount) = maybe_max_discountable_amount {
        if max_discountable_amount > discounted_price - undiscounted_price {
            discounted_price = undiscounted_price - max_discountable_amount;
        }
    }
    let total_price = discounted_price as u64 + shipment_fee;
    total_price
}
