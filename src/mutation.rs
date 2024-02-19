use std::any::type_name;
use std::collections::BTreeSet;

use async_graphql::{Context, Error, Object, Result};
use bson::Bson;
use bson::Uuid;
use futures::TryStreamExt;
use mongodb::{
    bson::{doc, DateTime},
    Collection, Database,
};
use serde::Deserialize;

use crate::authentication::authenticate_user;
use crate::foreign_types::Discount;
use crate::foreign_types::ProductItem;
use crate::foreign_types::ProductVariantVersion;
use crate::foreign_types::ShipmentMethod;
use crate::foreign_types::TaxRateVersion;
use crate::mutation_input_structs::CreateOrderInput;
use crate::mutation_input_structs::OrderItemInput;
use crate::order::OrderStatus;
use crate::order_item::OrderItem;
use crate::query::query_object;
use crate::user::User;
use crate::{order::Order, query::query_order};

/// Describes GraphQL order mutations.
pub struct Mutation;

#[Object]
impl Mutation {
    /// Creates an order with `OrderStatus::Pending`.
    async fn create_order<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "CreateOrderInput")] input: CreateOrderInput,
    ) -> Result<Order> {
        authenticate_user(&ctx, input.user_id)?;
        let db_client = ctx.data_unchecked::<Database>();
        let collection: Collection<Order> = db_client.collection::<Order>("orders");
        validate_input(db_client, &input).await?;
        let current_timestamp = DateTime::now();
        let internal_order_items = input
            .order_items
            .iter()
            .map(|i| OrderItem::new(i, current_timestamp))
            .collect();
        let order = Order {
            _id: Uuid::new(),
            user: User { _id: input.user_id },
            created_at: current_timestamp,
            order_status: OrderStatus::Pending,
            placed_at: None,
            rejection_reason: None,
            internal_order_items,
        };
        match collection.insert_one(order, None).await {
            Ok(result) => {
                let id = uuid_from_bson(result.inserted_id)?;
                query_order(&collection, id).await
            }
            Err(_) => Err(Error::new("Adding order failed in MongoDB.")),
        }
    }

    /// Places an existing order by changing its status to `OrderStatus::Placed`.
    async fn place_order<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "Uuid of order to place")] id: Uuid,
    ) -> Result<Order> {
        let db_client = ctx.data_unchecked::<Database>();
        let collection: Collection<Order> = db_client.collection::<Order>("orders");
        let order = query_order(&collection, id).await?;
        authenticate_user(&ctx, order.user._id)?;
        set_status_placed(&collection, id).await?;
        query_order(&collection, id).await
    }
}

/// Extracts UUID from Bson.
///
/// Adding a order returns a UUID in a Bson document. This function helps to extract the UUID.
fn uuid_from_bson(bson: Bson) -> Result<Uuid> {
    match bson {
        Bson::Binary(id) => Ok(id.to_uuid()?),
        _ => {
            let message = format!(
                "Returned id: `{}` needs to be a Binary in order to be parsed as a Uuid",
                bson
            );
            Err(Error::new(message))
        }
    }
}

/// Sets the status of an order to `OrderStatus::Placed`.
///
/// * `collection` - MongoDB collection to update.
/// * `input` - `UpdateOrderInput`.
async fn set_status_placed(collection: &Collection<Order>, id: Uuid) -> Result<()> {
    let current_timestamp = DateTime::now();
    let result = collection
        .update_one(
            doc! {"_id": id },
            doc! {"$set": {"order_status": OrderStatus::Placed, "placed_at": current_timestamp}},
            None,
        )
        .await;
    if let Err(_) = result {
        let message = format!("Placing order of id: `{}` failed in MongoDB.", id);
        return Err(Error::new(message));
    }
    Ok(())
}

/// Checks if foreign types exist (MongoDB database populated with events).
async fn validate_input(db_client: &Database, input: &CreateOrderInput) -> Result<()> {
    let user_collection: mongodb::Collection<User> = db_client.collection::<User>("users");
    validate_object(&user_collection, input.user_id).await?;
    validate_order_items(&db_client, &input.order_items).await?;
    Ok(())
}

/// Checks if all order item parameters are the system (MongoDB database populated with events).
///
/// Used before creating orders.
async fn validate_order_items(
    db_client: &Database,
    order_items: &BTreeSet<OrderItemInput>,
) -> Result<()> {
    let product_variant_version_collection: mongodb::Collection<ProductVariantVersion> =
        db_client.collection::<ProductVariantVersion>("product_variant_versions");
    let product_item_collection: mongodb::Collection<ProductItem> =
        db_client.collection::<ProductItem>("product_items");
    let tax_rate_version_collection: mongodb::Collection<TaxRateVersion> =
        db_client.collection::<TaxRateVersion>("tax_rate_versions");
    let shipment_method_collection: mongodb::Collection<ShipmentMethod> =
        db_client.collection::<ShipmentMethod>("shipment_methods");
    let product_variant_version_ids = order_items
        .iter()
        .map(|o| o.product_variant_version_id)
        .collect();
    let product_item_ids = order_items.iter().map(|o| o.product_item_id).collect();
    let tax_rate_version_ids = order_items.iter().map(|o| o.tax_rate_version_id).collect();
    let shipment_method_ids = order_items.iter().map(|o| o.shipment_method_id).collect();
    validate_objects(
        &product_variant_version_collection,
        product_variant_version_ids,
    )
    .await?;
    validate_objects(&product_item_collection, product_item_ids).await?;
    validate_objects(&tax_rate_version_collection, tax_rate_version_ids).await?;
    validate_objects(&shipment_method_collection, shipment_method_ids).await?;
    validate_discounts(&db_client, &order_items).await?;
    Ok(())
}

/// Checks if discounts are in the system (MongoDB database populated with events).
///
/// Used before creating orders.
async fn validate_discounts(
    db_client: &Database,
    order_items: &BTreeSet<OrderItemInput>,
) -> Result<()> {
    let discount_collection: mongodb::Collection<Discount> =
        db_client.collection::<Discount>("discounts");
    let discount_ids: Vec<Uuid> = order_items
        .iter()
        .map(|o| o.discounts.clone())
        .flatten()
        .collect();
    validate_objects(&discount_collection, discount_ids).await
}

/// Checks if a single object is in the system (MongoDB database populated with events).
///
/// Used before creating orders.
async fn validate_object<T: for<'a> Deserialize<'a> + Unpin + Send + Sync>(
    collection: &Collection<T>,
    id: Uuid,
) -> Result<()> {
    query_object(&collection, id).await.map(|_| ())
}

/// Checks if all objects are in the system (MongoDB database populated with events).
///
/// Used before creating orders.
async fn validate_objects<
    T: for<'b> Deserialize<'b> + Unpin + Send + Sync + PartialEq + From<Uuid>,
>(
    collection: &Collection<T>,
    object_ids: Vec<Uuid>,
) -> Result<()> {
    match collection
        .find(doc! {"_id": { "$in": &object_ids } }, None)
        .await
    {
        Ok(cursor) => {
            let product_variants: Vec<T> = cursor.try_collect().await?;
            object_ids.iter().fold(Ok(()), |o, p| {
                let object = T::from(*p);
                match product_variants.contains(&object) {
                    true => o.and(Ok(())),
                    false => {
                        let message = format!(
                            "{} with UUID: `{}` is not present in the system.",
                            type_name::<T>(),
                            p
                        );
                        Err(Error::new(message))
                    }
                }
            })
        }
        Err(_) => {
            let message = format!(
                "{} with specified UUIDs are not present in the system.",
                type_name::<T>()
            );
            Err(Error::new(message))
        }
    }
}
