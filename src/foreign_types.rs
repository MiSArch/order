use async_graphql::SimpleObject;
use bson::{doc, Bson, Uuid};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, hash::Hash};

use crate::http_event_service::ProductVariantVersionEventData;

/// Foreign type of a product variant.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct ProductVariantVersion {
    /// UUID of the product variant version.
    pub _id: Uuid,
    /// Price of the product variant version.
    pub price: u64,
    /// UUID of tax rate version associated with order item.
    pub tax_rate_version_id: Uuid,
}

impl PartialOrd for ProductVariantVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl From<ProductVariantVersionEventData> for ProductVariantVersion {
    fn from(value: ProductVariantVersionEventData) -> Self {
        Self {
            _id: value.id,
            price: value.price,
            tax_rate_version_id: value.tax_rate_version_id,
        }
    }
}

impl From<ProductVariantVersion> for Bson {
    fn from(value: ProductVariantVersion) -> Self {
        Bson::Document(doc!("_id": value._id))
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

/// Foreign type of a discount.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct Discount {
    /// UUID of the discount.
    pub _id: Uuid,
}

impl PartialOrd for Discount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
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

/// Foreign type of a shipment method.
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
