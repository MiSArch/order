use axum::{debug_handler, extract::State, http::StatusCode, Json};
use bson::Uuid;
use log::info;
use mongodb::Collection;
use serde::{Deserialize, Serialize};

use crate::{
    foreign_types::{Coupon, ProductVariantVersion, ShipmentMethod},
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
pub struct Event<T> {
    pub topic: String,
    pub data: T,
}

/// Event data containing a Uuid.
#[derive(Deserialize, Debug)]
pub struct UuidEventData {
    pub id: Uuid,
}

/// Event data containing a ProductVariantVersion.
///
/// Differs from ProductVariantVersion in the `id` field naming.
#[derive(Deserialize, Debug)]
pub struct ProductVariantVersionEventData {
    /// UUID of the product variant version.
    pub id: Uuid,
    /// Price of the product variant version.
    pub price: u64,
    /// UUID of tax rate version associated with order item.
    pub tax_rate_version_id: Uuid,
}

/// Service state containing database connections.
#[derive(Clone)]
pub struct HttpEventServiceState {
    pub product_variant_version_collection: Collection<ProductVariantVersion>,
    pub coupon_collection: Collection<Coupon>,
    pub shipment_method_collection: Collection<ShipmentMethod>,
    pub user_collection: Collection<User>,
}

/// HTTP endpoint to list topic subsciptions.
pub async fn list_topic_subscriptions() -> Result<Json<Vec<Pubsub>>, StatusCode> {
    let pubsub_product_variant_version = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "catalog/product-variant-version/created".to_string(),
        route: "/on-product-variant-version-creation-event".to_string(),
    };
    let pubsub_coupon = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "discount/coupon/created".to_string(),
        route: "/on-id-creation-event".to_string(),
    };
    let pubsub_shipment_method = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "shipment/shipment-method/created".to_string(),
        route: "/on-id-creation-event".to_string(),
    };
    let pubsub_user = Pubsub {
        pubsubname: "pubsub".to_string(),
        topic: "user/user/created".to_string(),
        route: "/on-id-creation-event".to_string(),
    };
    Ok(Json(vec![
        pubsub_product_variant_version,
        pubsub_coupon,
        pubsub_shipment_method,
        pubsub_user,
    ]))
}

/// HTTP endpoint to receive UUID creation events.
///
/// Includes all creation events that consist of only UUIDs:
/// - Coupon
/// - ShipmentMethod
/// - User
#[debug_handler(state = HttpEventServiceState)]
pub async fn on_id_creation_event(
    State(state): State<HttpEventServiceState>,
    Json(event): Json<Event<UuidEventData>>,
) -> Result<Json<TopicEventResponse>, StatusCode> {
    info!("{:?}", event);

    match event.topic.as_str() {
        "discount/coupon/created" => {
            add_to_mongodb(&state.coupon_collection, event.data.id).await?
        }
        "shipment/shipment-method/created" => {
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

/// HTTP endpoint to receive ProductVariantVersion creation events.
#[debug_handler(state = HttpEventServiceState)]
pub async fn on_product_variant_version_creation_event(
    State(state): State<HttpEventServiceState>,
    Json(event): Json<Event<ProductVariantVersionEventData>>,
) -> Result<Json<TopicEventResponse>, StatusCode> {
    info!("{:?}", event);

    let product_variant_version = ProductVariantVersion::from(event.data);
    match event.topic.as_str() {
        "catalog/product-variant-version/created" => {
            add_product_variant_version_to_mongodb(
                &state.product_variant_version_collection,
                product_variant_version,
            )
            .await?
        }
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

/// Add a newly created ProductVariantVersion to MongoDB.
pub async fn add_product_variant_version_to_mongodb(
    collection: &Collection<ProductVariantVersion>,
    product_variant_version: ProductVariantVersion,
) -> Result<(), StatusCode> {
    match collection.insert_one(product_variant_version, None).await {
        Ok(_) => Ok(()),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Add a newly created object: T to MongoDB.
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
