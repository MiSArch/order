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
use crate::mutation_input_structs::CreateOrderInput;
use crate::order::OrderStatus;
use crate::query::query_user;
use crate::user::User;
use crate::{
    query::query_order,
    order::Order,
};

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
        let normalized_product_variants: HashSet<ProductVariant> = input
            .product_variant_ids
            .iter()
            .map(|id| ProductVariant { _id: id.clone() })
            .collect();
        let current_timestamp = DateTime::now();
        let internal_order_items = input.order_items.iter().map{|x| OrderItem::new(x, &current_timestamp)}.collect();
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

    /// Updates name and/or product_variant_ids of a specific order referenced with an id.
    ///
    /// Formats UUIDs as hyphenated lowercase Strings.
    async fn update_order<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "UpdateOrderInput")] input: UpdateOrderInput,
    ) -> Result<Order> {
        let db_client = ctx.data_unchecked::<Database>();
        let collection: Collection<Order> = db_client.collection::<Order>("orders");
        let order = query_order(&collection, input.id).await?;
        authenticate_user(&ctx, order.user._id)?;
        let product_variant_collection: Collection<ProductVariant> =
            db_client.collection::<ProductVariant>("product_variants");
        let current_timestamp = DateTime::now();
        update_product_variant_ids(
            &collection,
            &product_variant_collection,
            &input,
            &current_timestamp,
        )
        .await?;
        update_name(&collection, &input, &current_timestamp).await?;
        query_order(&collection, input.id).await
    }

    /// Deletes order of id.
    async fn delete_order<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "UUID of order to delete.")] id: Uuid,
    ) -> Result<bool> {
        let db_client = ctx.data_unchecked::<Database>();
        let collection: Collection<Order> = db_client.collection::<Order>("orders");
        let order = query_order(&collection, id).await?;
        authenticate_user(&ctx, order.user._id)?;
        if let Err(_) = collection.delete_one(doc! {"_id": id }, None).await {
            let message = format!("Deleting order of id: `{}` failed in MongoDB.", id);
            return Err(Error::new(message));
        }
        Ok(true)
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

/// Updates product variant ids of a order.
///
/// * `collection` - MongoDB collection to update.
/// * `input` - `UpdateOrderInput`.
async fn update_product_variant_ids(
    collection: &Collection<Order>,
    product_variant_collection: &Collection<ProductVariant>,
    input: &UpdateOrderInput,
    current_timestamp: &DateTime,
) -> Result<()> {
    if let Some(definitely_product_variant_ids) = &input.product_variant_ids {
        validate_product_variant_ids(&product_variant_collection, definitely_product_variant_ids)
            .await?;
        let normalized_product_variants: Vec<ProductVariant> = definitely_product_variant_ids
            .iter()
            .map(|id| ProductVariant { _id: id.clone() })
            .collect();
        if let Err(_) = collection.update_one(doc!{"_id": input.id }, doc!{"$set": {"internal_product_variants": normalized_product_variants, "last_updated_at": current_timestamp}}, None).await {
            let message = format!("Updating product_variant_ids of order of id: `{}` failed in MongoDB.", input.id);
            return Err(Error::new(message))
        }
    }
    Ok(())
}

/// Updates name of a order.
///
/// * `collection` - MongoDB collection to update.
/// * `input` - `UpdateOrderInput`.
async fn update_name(
    collection: &Collection<Order>,
    input: &UpdateOrderInput,
    current_timestamp: &DateTime,
) -> Result<()> {
    if let Some(definitely_name) = &input.name {
        let result = collection
            .update_one(
                doc! {"_id": input.id },
                doc! {"$set": {"name": definitely_name, "last_updated_at": current_timestamp}},
                None,
            )
            .await;
        if let Err(_) = result {
            let message = format!(
                "Updating name of order of id: `{}` failed in MongoDB.",
                input.id
            );
            return Err(Error::new(message));
        }
    }
    Ok(())
}

/// Checks if product variants and user in AddOrderInput are in the system (MongoDB database populated with events).
async fn validate_input(db_client: &Database, input: &AddOrderInput) -> Result<()> {
    let product_variant_collection: Collection<ProductVariant> =
        db_client.collection::<ProductVariant>("product_variants");
    let user_collection: Collection<User> = db_client.collection::<User>("users");
    validate_product_variant_ids(&product_variant_collection, &input.product_variant_ids).await?;
    validate_user(&user_collection, input.user_id).await?;
    Ok(())
}

/// Checks if product variants are in the system (MongoDB database populated with events).
///
/// Used before adding or modifying product variants / orders.
async fn validate_product_variant_ids(
    collection: &Collection<ProductVariant>,
    product_variant_ids: &HashSet<Uuid>,
) -> Result<()> {
    let product_variant_ids_vec: Vec<Uuid> = product_variant_ids.clone().into_iter().collect();
    match collection
        .find(doc! {"_id": { "$in": &product_variant_ids_vec } }, None)
        .await
    {
        Ok(cursor) => {
            let product_variants: Vec<ProductVariant> = cursor.try_collect().await?;
            product_variant_ids_vec.iter().fold(Ok(()), |_, p| {
                match product_variants.contains(&ProductVariant { _id: *p }) {
                    true => Ok(()),
                    false => {
                        let message = format!(
                            "Product variant with the UUID: `{}` is not present in the system.",
                            p
                        );
                        Err(Error::new(message))
                    }
                }
            })
        }
        Err(_) => Err(Error::new(
            "Product variants with the specified UUIDs are not present in the system.",
        )),
    }
}

/// Checks if user is in the system (MongoDB database populated with events).
///
/// Used before adding orders.
async fn validate_user(collection: &Collection<User>, id: Uuid) -> Result<()> {
    query_user(&collection, id).await.map(|_| ())
}
