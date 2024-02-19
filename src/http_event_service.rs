use axum::{debug_handler, extract::State, http::StatusCode, Json};
use bson::Uuid;
use log::info;
use mongodb::Collection;
use serde::{Deserialize, Serialize};

use crate::{
    foreign_types::{Discount, ProductItem, ProductVariantVersion, ShipmentMethod, TaxRateVersion},
    user::User,
};

/// Data to send to Dapr in order to describe a subscription.
#[derive(Serialize)]
pub struct Pubsub {
    #[serde(rename(serialize = "pubsubName"))]
    pub pubsubname: String,
    pub topic: String,
    pub route: String,
}

/// Reponse data to send to Dapr when receiving an event.
#[derive(Serialize)]
pub struct TopicEventResponse {
    pub status: i32,
}

/// Default status is `0` -> Ok, according to Dapr specs.
impl Default for TopicEventResponse {
    fn default() -> Self {
        Self { status: 0 }
    }
}

/// Relevant part of Dapr event wrapped in a CloudEnvelope.
#[derive(Deserialize, Debug)]
pub struct Event {
    pub topic: String,
    pub data: EventData,
}

/// Relevant part of Dapr event.data.
#[derive(Deserialize, Debug)]
pub struct EventData {
    pub id: Uuid,
}

/// Service state containing database connections.
#[derive(Clone)]
pub struct HttpEventServiceState {
    pub product_variant_version_collection: Collection<ProductVariantVersion>,
    pub product_item_collection: Collection<ProductItem>,
    pub tax_rate_version_collection: Collection<TaxRateVersion>,
    pub discount_collection: Collection<Discount>,
    pub shipment_method_collection: Collection<ShipmentMethod>,
    pub user_collection: Collection<User>,
}

/// HTTP endpoint to list topic subsciptions.
pub async fn list_topic_subscriptions() -> Result<Json<Vec<Pubsub>>, StatusCode> {
    let pubsub_product_variant_version = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "catalog/product-variant/created".to_string(),
        route: "/on-topic-event".to_string(),
    };
    let pubsub_product_item = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "inventory/product_item/created".to_string(),
        route: "/on-topic-event".to_string(),
    };
    let pubsub_tax_rate_version = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "tax/tax_rate_version/created".to_string(),
        route: "/on-topic-event".to_string(),
    };
    let pubsub_discount = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "discount/discount/created".to_string(),
        route: "/on-topic-event".to_string(),
    };
    let pubsub_shipment_method = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "shipment/shipment_method/created".to_string(),
        route: "/on-topic-event".to_string(),
    };
    let pubsub_user = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "user/user/created".to_string(),
        route: "/on-topic-event".to_string(),
    };
    Ok(Json(vec![
        pubsub_product_variant_version,
        pubsub_product_item,
        pubsub_tax_rate_version,
        pubsub_discount,
        pubsub_shipment_method,
        pubsub_user,
    ]))
}

/// HTTP endpoint to receive events.
#[debug_handler(state = HttpEventServiceState)]
pub async fn on_topic_event(
    State(state): State<HttpEventServiceState>,
    Json(event): Json<Event>,
) -> Result<Json<TopicEventResponse>, StatusCode> {
    info!("{:?}", event);

    match event.topic.as_str() {
        "catalog/product-variant-version/created" => {
            add_to_mongodb(&state.product_variant_version_collection, event.data.id).await?
        }
        "inventory/product_item/created" => {
            add_to_mongodb(&state.product_item_collection, event.data.id).await?
        }
        "tax/tax_rate_version/created" => {
            add_to_mongodb(&state.tax_rate_version_collection, event.data.id).await?
        }
        "discount/discount/created" => {
            add_to_mongodb(&state.discount_collection, event.data.id).await?
        }
        "shipment/shipment_method/created" => {
            add_to_mongodb(&state.shipment_method_collection, event.data.id).await?
        }
        "user/user/created" => add_to_mongodb(&state.user_collection, event.data.id).await?,
        _ => {
            // TODO: This message can be used for further Error visibility.
            let _message = format!(
                "Event of topic: `{}` is not a handleable by this service.",
                event.topic.as_str()
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
    Ok(Json(TopicEventResponse::default()))
}

/// Add a newly created object T to MongoDB.
pub async fn add_to_mongodb<T: Serialize + From<Uuid>>(
    collection: &Collection<T>,
    id: Uuid,
) -> Result<(), StatusCode> {
    let object = T::from(id);
    match collection.insert_one(object, None).await {
        Ok(_) => Ok(()),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
