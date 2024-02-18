use axum::{debug_handler, extract::State, http::StatusCode, Json};
use bson::Uuid;
use log::info;
use mongodb::Collection;
use serde::{Deserialize, Serialize};

use crate::{foreign_types::ProductVariant, user::User};

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
    pub product_variant_collection: Collection<ProductVariant>,
    pub user_collection: Collection<User>,
}

/// HTTP endpoint to list topic subsciptions.
pub async fn list_topic_subscriptions() -> Result<Json<Vec<Pubsub>>, StatusCode> {
    let pubsub_user = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "user/user/created".to_string(),
        route: "/on-topic-event".to_string(),
    };
    let pubsub_product_variant = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "catalog/product-variant/created".to_string(),
        route: "/on-topic-event".to_string(),
    };
    Ok(Json(vec![pubsub_user, pubsub_product_variant]))
}

/// HTTP endpoint to receive events.
#[debug_handler(state = HttpEventServiceState)]
pub async fn on_topic_event(
    State(state): State<HttpEventServiceState>,
    Json(event): Json<Event>,
) -> Result<Json<TopicEventResponse>, StatusCode> {
    info!("{:?}", event);

    match event.topic.as_str() {
        "catalog/product-variant/created" => {
            add_product_variant_to_mongodb(state.product_variant_collection, event.data.id).await?
        }
        "user/user/created" => add_user_to_mongodb(state.user_collection, event.data.id).await?,
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

/// Add a newly created product variant to MongoDB.
pub async fn add_product_variant_to_mongodb(
    collection: Collection<ProductVariant>,
    id: Uuid,
) -> Result<(), StatusCode> {
    let product_variant = ProductVariant { _id: id };
    match collection.insert_one(product_variant, None).await {
        Ok(_) => Ok(()),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Add a newly created user to MongoDB.
pub async fn add_user_to_mongodb(collection: Collection<User>, id: Uuid) -> Result<(), StatusCode> {
    let user = User { _id: id };
    match collection.insert_one(user, None).await {
        Ok(_) => Ok(()),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
