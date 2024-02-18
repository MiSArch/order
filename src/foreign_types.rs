use async_graphql::SimpleObject;
use bson::{doc, Bson, Uuid};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, hash::Hash};

/// Foreign type of a product variant.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct ProductVariantVersion {
    /// UUID of the product variant version.
    pub _id: Uuid,
}

impl PartialOrd for ProductVariantVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl From<ProductVariantVersion> for Bson {
    fn from(value: ProductVariantVersion) -> Self {
        Bson::Document(doc!("_id": value._id))
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

/// Foreign type of a tax rate version.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct TaxRateVersion {
    /// UUID of the tax rate version.
    pub _id: Uuid,
}

impl PartialOrd for TaxRateVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl From<TaxRateVersion> for Bson {
    fn from(value: TaxRateVersion) -> Self {
        Bson::Document(doc!("_id": value._id))
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

