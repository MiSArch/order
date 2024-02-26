use std::cmp::Ordering;

use async_graphql::{Enum, SimpleObject};
use bson::{doc, Bson, Uuid};
use serde::{Deserialize, Serialize};

/// Defines a shipment associated with one or more order items.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone, SimpleObject)]
#[graphql(unresolvable)]
pub struct Shipment {
    /// UUID of the shipment.
    pub _id: Uuid,
    /// Shipment status of the shipment.
    pub status: ShipmentStatus,
    /// Method/provider, which is used for shipping.
    pub shipment_method: ShipmentMethod,
}

impl PartialOrd for Shipment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self._id.partial_cmp(&other._id)
    }
}

impl From<Shipment> for Bson {
    fn from(value: Shipment) -> Self {
        Bson::Document(doc!("_id": value._id))
    }
}

impl From<Shipment> for Uuid {
    fn from(value: Shipment) -> Self {
        value._id
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

/// Defines the shipments delivery and return status.
#[derive(Debug, Serialize, Deserialize, Enum, Hash, PartialEq, Eq, Clone, Copy)]
pub enum ShipmentStatus {
    Pending,
    InProgress,
    Delivered,
    Returned,
    ReturnInProgress,
}
