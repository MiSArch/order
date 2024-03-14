use async_graphql::{Error, Result};
use bson::{doc, DateTime, Uuid};
use futures::TryStreamExt;
use mongodb::Collection;
use serde::{Deserialize, Serialize};

use crate::{
    http_event_service::ShipmentFailedEventData, mutation::validate_object, order::Order,
    query::query_object,
};

/// Models an order compensation that is sent as an event and logged in MongoDB.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderCompensation {
    /// OrderCompensation UUID.
    pub _id: Uuid,
    /// UUID of the order.
    pub order_id: Uuid,
    /// UUIDs of the order items of shipment.
    pub order_item_ids: Vec<Uuid>,
    /// Timestamp when compensation was triggered.
    pub triggered_at: DateTime,
    /// Amount of order compensation
    pub amount_to_compensate: u64,
}

/// DTO that models an order compensation that is sent as an event and logged in MongoDB.
#[derive(Debug, Serialize)]
pub struct OrderCompensationDTO {
    /// OrderCompensation UUID.
    pub id: Uuid,
    /// Amount of order compensation
    pub amount_to_compensate: u64,
}

impl From<OrderCompensation> for OrderCompensationDTO {
    fn from(value: OrderCompensation) -> Self {
        Self {
            id: value._id,
            amount_to_compensate: value.amount_to_compensate,
        }
    }
}

pub async fn compensate_order(
    order_collection: &Collection<Order>,
    order_compensation_collection: &Collection<OrderCompensation>,
    data: ShipmentFailedEventData,
) -> Result<()> {
    validate_object(&order_collection, data.order_id).await?;
    verify_items_uncompensated(&order_compensation_collection, &data.order_item_ids).await?;
    let amount_to_compensate = calculate_amount_to_compensate(&order_collection, &data).await?;
    let order_compensation = OrderCompensation {
        _id: Uuid::new(),
        order_id: data.order_id,
        order_item_ids: data.order_item_ids,
        triggered_at: DateTime::now(),
        amount_to_compensate,
    };
    insert_order_compensation_in_mongodb(&order_compensation_collection, &order_compensation)
        .await?;
    send_order_compensation_event(order_compensation).await
}

async fn calculate_amount_to_compensate(
    order_collection: &Collection<Order>,
    data: &ShipmentFailedEventData,
) -> Result<u64> {
    let order = query_object(&order_collection, data.order_id).await?;
    let compensatable_amounts: Vec<u64> = order
        .internal_order_items
        .iter()
        .filter(|i| data.order_item_ids.contains(&i._id))
        .map(|i| i.compensatable_amount)
        .collect();
    let amount_to_compensate = compensatable_amounts.iter().sum();
    Ok(amount_to_compensate)
}

async fn verify_items_uncompensated(
    order_collection: &Collection<OrderCompensation>,
    order_item_ids: &Vec<Uuid>,
) -> Result<()> {
    let query = doc! {"order_item_ids": {"$not": {"$elemMatch": {"$in": order_item_ids}}}};
    let message = format!(
        "Order items of UUIDs: `{:?}` could not be verfied.",
        order_item_ids
    );
    match order_collection.find(query, None).await {
        Ok(cursor) => {
            let objects: Vec<OrderCompensation> = cursor.try_collect().await?;
            match objects.len() {
                0 => Ok(()),
                _ => Err(Error::new(message)),
            }
        }
        Err(_) => Err(Error::new(message)),
    }
}

async fn insert_order_compensation_in_mongodb(
    order_collection: &Collection<OrderCompensation>,
    order_compensation: &OrderCompensation,
) -> Result<()> {
    match order_collection.insert_one(order_compensation, None).await {
        Ok(_) => Ok(()),
        Err(_) => Err(Error::new("Adding order compensation failed in MongoDB.")),
    }
}

/// Sends an `order/order/compensate` created event containing the order context.
async fn send_order_compensation_event(order_compensation: OrderCompensation) -> Result<()> {
    let client = reqwest::Client::new();
    let order_compensation_dto = OrderCompensationDTO::from(order_compensation);
    client
        .post("http://localhost:3500/v1.0/publish/order/order/created")
        .json(&order_compensation_dto)
        .send()
        .await?;
    Ok(())
}
