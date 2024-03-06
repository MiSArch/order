use async_graphql::{Error, Result};
use bson::{doc, DateTime, Uuid};
use mongodb::Collection;
use serde::Serialize;

use crate::http_event_service::ShipmentFailedEventData;

/// Models an order compensation that is sent as an event and logged in MongoDB.
#[derive(Debug, Serialize, Clone)]
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
    order_collection: &Collection<OrderCompensation>,
    data: ShipmentFailedEventData,
) -> Result<()> {
    verify_items_uncompensated(&order_collection, &data.order_item_ids).await?;
    let order_compensation = OrderCompensation {
        _id: Uuid::new(),
        order_id: data.order_id,
        order_item_ids: data.order_item_ids,
        triggered_at: DateTime::now(),
        amount_to_compensate: todo!(),
    };
    insert_order_compensation_in_mongodb(&order_collection, &order_compensation).await?;
    send_order_compensation_event(order_compensation).await
}

async fn verify_items_uncompensated(
    order_collection: &Collection<OrderCompensation>,
    order_item_ids: &Vec<Uuid>,
) -> Result<()> {
    let pipeline = vec![
        doc! {
            "$match": {
              "order_item_ids": {
                "$not": {
                  "$in": order_item_ids
                }
              }
            }
        },
        doc! {
            "$group": {
              "_id": None::<Uuid>,
              "count": { "$sum": 1 }
            }
        },
    ];
    todo!()
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
