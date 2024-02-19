use std::collections::HashSet;

use async_graphql::{Context, Error, Object, Result};
use bson::Bson;
use bson::Uuid;
use futures::TryStreamExt;
use mongodb::{
    bson::{doc, DateTime},
    Collection, Database,
};

use crate::authentication::authenticate_user;
use crate::foreign_types::ProductVariantVersion;
use crate::mutation_input_structs::CreateOrderInput;
use crate::order::OrderStatus;
use crate::order_item::OrderItem;
use crate::query::query_user;
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
    let result = collection
        .update_one(
            doc! {"_id": id },
            doc! {"$set": {"status": OrderStatus::Placed}},
            None,
        )
        .await;
    if let Err(_) = result {
        let message = format!("Placing order of id: `{}` failed in MongoDB.", id);
        return Err(Error::new(message));
    }
    Ok(())
}

/// Checks if product variants versions and user in CreateOrderInput are in the system (MongoDB database populated with events).
async fn validate_input(db_client: &Database, input: &CreateOrderInput) -> Result<()> {
    let product_variant_collection: Collection<ProductVariantVersion> =
        db_client.collection::<ProductVariantVersion>("product_variant_versions");
    let user_collection: Collection<User> = db_client.collection::<User>("users");
    validate_product_variant_version_ids(&product_variant_collection, &input.product_variant_ids)
        .await?;
    validate_user(&user_collection, input.user_id).await?;
    Ok(())
}

/// Checks if product variants versions are in the system (MongoDB database populated with events).
///
/// Used before creating orders.
async fn validate_product_variant_version_ids(
    collection: &Collection<ProductVariantVersion>,
    product_variant_ids: &HashSet<Uuid>,
) -> Result<()> {
    let product_variant_ids_vec: Vec<Uuid> = product_variant_ids.clone().into_iter().collect();
    match collection
        .find(doc! {"_id": { "$in": &product_variant_ids_vec } }, None)
        .await
    {
        Ok(cursor) => {
            let product_variants: Vec<ProductVariantVersion> = cursor.try_collect().await?;
            product_variant_ids_vec.iter().fold(Ok(()), |_, p| {
                match product_variants.contains(&ProductVariantVersion { _id: *p }) {
                    true => Ok(()),
                    false => {
                        let message = format!(
                            "Product variant version with the UUID: `{}` is not present in the system.",
                            p
                        );
                        Err(Error::new(message))
                    }
                }
            })
        }
        Err(_) => Err(Error::new(
            "Product variant versions with the specified UUIDs are not present in the system.",
        )),
    }
}

/// Checks if user is in the system (MongoDB database populated with events).
///
/// Used before creating orders.
async fn validate_user(collection: &Collection<User>, id: Uuid) -> Result<()> {
    query_user(&collection, id).await.map(|_| ())
}
