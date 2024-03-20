use async_graphql::SimpleObject;
use bson::{doc, Bson, Uuid};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, hash::Hash};

use crate::http_event_service::{ProductVariantVersionEventData, TaxRateVersionEventData};
use crate::mutation::get_discounts;

/// Foreign type of a product variant.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct ProductVariant {
    /// UUID of the product variant.
    pub _id: Uuid,
    /// Current version of product variant.
    pub current_version: ProductVariantVersion,
    /// Defines visibility of product variant.
    pub is_publicly_visible: bool,
}

impl From<ProductVariantVersionEventData> for ProductVariant {
    fn from(value: ProductVariantVersionEventData) -> Self {
        Self {
            _id: value.product_variant_id,
            current_version: ProductVariantVersion::from(value),
            is_publicly_visible: true,
        }
    }
}

impl PartialOrd for ProductVariant {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl From<ProductVariant> for Uuid {
    fn from(value: ProductVariant) -> Self {
        value._id
    }
}

/// Foreign type of a product variant.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct ProductVariantVersion {
    /// UUID of the product variant version.
    pub _id: Uuid,
    /// Price of the product variant version.
    pub price: u32,
    /// UUID of tax rate associated with order item.
    pub tax_rate_id: Uuid,
}

impl From<ProductVariantVersionEventData> for ProductVariantVersion {
    fn from(value: ProductVariantVersionEventData) -> Self {
        Self {
            _id: value.id,
            price: value.retail_price,
            tax_rate_id: value.tax_rate_id,
        }
    }
}

impl PartialOrd for ProductVariantVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl From<ProductVariantVersion> for Bson {
    fn from(value: ProductVariantVersion) -> Self {
        Bson::Document(doc!{"_id": value._id, "price": value.price, "tax_rate_id": value.tax_rate_id})
    }
}

impl From<ProductVariantVersion> for Uuid {
    fn from(value: ProductVariantVersion) -> Self {
        value._id
    }
}

/// Foreign type of a product item.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct ProductItem {
    /// UUID of the product item.
    pub _id: Uuid,
}

impl PartialOrd for ProductItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl From<ProductItem> for Bson {
    fn from(value: ProductItem) -> Self {
        Bson::Document(doc!("_id": value._id))
    }
}

impl From<ProductItem> for Uuid {
    fn from(value: ProductItem) -> Self {
        value._id
    }
}

/// Foreign type of a coupon.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct Coupon {
    /// UUID of the coupon.
    pub _id: Uuid,
}

impl PartialOrd for Coupon {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl From<Coupon> for Bson {
    fn from(value: Coupon) -> Self {
        Bson::Document(doc!("_id": value._id))
    }
}

impl From<Coupon> for Uuid {
    fn from(value: Coupon) -> Self {
        value._id
    }
}

impl From<Uuid> for Coupon {
    fn from(value: Uuid) -> Self {
        Coupon { _id: value }
    }
}

/// Foreign type of a tax rate.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct TaxRate {
    /// UUID of the tax rate.
    pub _id: Uuid,
    /// Current version of tax rate.
    pub current_version: TaxRateVersion,
}

impl From<TaxRateVersionEventData> for TaxRate {
    fn from(value: TaxRateVersionEventData) -> Self {
        Self {
            _id: value.tax_rate_id,
            current_version: TaxRateVersion::from(value),
        }
    }
}

impl From<TaxRate> for Bson {
    fn from(value: TaxRate) -> Self {
        let current_version_bson = Bson::from(value.current_version);
        Bson::Document(doc!("_id": value._id, "current_version": current_version_bson))
    }
}

impl From<TaxRate> for Uuid {
    fn from(value: TaxRate) -> Self {
        value._id
    }
}

/// Foreign type of a tax rate version.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct TaxRateVersion {
    /// UUID of the tax rate.
    pub _id: Uuid,
    /// Rate of the tax rate version.
    pub rate: f64,
    /// Version number of product variant version.
    pub version: u32,
}

impl From<TaxRateVersionEventData> for TaxRateVersion {
    fn from(value: TaxRateVersionEventData) -> Self {
        Self {
            _id: value.id,
            rate: value.rate,
            version: value.version,
        }
    }
}

impl From<TaxRateVersion> for Bson {
    fn from(value: TaxRateVersion) -> Self {
        Bson::Document(doc!("_id": value._id, "rate": value.rate, "version": value.version))
    }
}

impl PartialEq for TaxRateVersion {
    fn eq(&self, other: &Self) -> bool {
        self._id == other._id
    }
}

impl Eq for TaxRateVersion {}

/// Foreign type of a discount.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct Discount {
    /// UUID of the discount.
    pub _id: Uuid,
    /// Amount to be discounted.
    pub discount: f64,
}

impl Ord for Discount {
    fn cmp(&self, other: &Self) -> Ordering {
        self._id.cmp(&other._id)
    }
}

impl PartialOrd for Discount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Discount> for Bson {
    fn from(value: Discount) -> Self {
        Bson::Document(doc!("_id": value._id))
    }
}

impl From<Discount> for Uuid {
    fn from(value: Discount) -> Self {
        value._id
    }
}

impl PartialEq for Discount {
    fn eq(&self, other: &Self) -> bool {
        self._id == other._id
    }
}

impl Eq for Discount {}

impl From<get_discounts::GetDiscountsFindApplicableDiscountsDiscounts> for Discount {
    fn from(value: get_discounts::GetDiscountsFindApplicableDiscountsDiscounts) -> Self {
        Self {
            _id: value.id,
            discount: value.discount,
        }
    }
}

/// Foreign type of a shopping cart item.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct ShoppingCartItem {
    /// UUID of the shopping cart item.
    pub _id: Uuid,
}

/// Foreign type of an address.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct Address {
    /// UUID of the product item.
    pub _id: Uuid,
}

impl PartialOrd for Address {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl From<Address> for Bson {
    fn from(value: Address) -> Self {
        Bson::Document(doc!("_id": value._id))
    }
}

impl From<Address> for Uuid {
    fn from(value: Address) -> Self {
        value._id
    }
}

impl From<Uuid> for Address {
    fn from(value: Uuid) -> Self {
        Address { _id: value }
    }
}

/// Describes the method/provider that the shipment uses.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct ShipmentMethod {
    /// UUID of the shipment method.
    pub _id: Uuid,
}

impl PartialOrd for ShipmentMethod {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl From<ShipmentMethod> for Bson {
    fn from(value: ShipmentMethod) -> Self {
        Bson::Document(doc!("_id": value._id))
    }
}

impl From<ShipmentMethod> for Uuid {
    fn from(value: ShipmentMethod) -> Self {
        value._id
    }
}

impl From<Uuid> for ShipmentMethod {
    fn from(value: Uuid) -> Self {
        ShipmentMethod { _id: value }
    }
}
