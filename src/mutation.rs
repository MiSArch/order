use async_graphql::{Context, Error, Object, Result};
use bson::Bson;
use bson::Uuid;
use futures::TryStreamExt;
use graphql_client::GraphQLQuery;
use graphql_client::Response;
use mongodb::{
    bson::{doc, DateTime},
    Collection, Database,
};
use serde::Deserialize;
use std::any::type_name;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::time::Duration;
use std::time::SystemTime;

use crate::authentication::authenticate_user;
use crate::foreign_types::Coupon;
use crate::foreign_types::Discount;
use crate::foreign_types::ProductItem;
use crate::foreign_types::ProductVariant;
use crate::foreign_types::ProductVariantVersion;
use crate::foreign_types::ShipmentMethod;
use crate::foreign_types::TaxRate;
use crate::foreign_types::TaxRateVersion;
use crate::mutation_input_structs::CreateOrderInput;
use crate::mutation_input_structs::OrderItemInput;
use crate::order::OrderStatus;
use crate::order_item::OrderItem;
use crate::query::query_object;
use crate::query::query_objects;
use crate::user::User;
use crate::{order::Order, query::query_order};

const PENDING_TIMEOUT: Duration = Duration::new(3600, 0);

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
        let internal_order_items =
            create_internal_order_items(&db_client, input.order_item_inputs, current_timestamp)
                .await?;
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
/// Creating a order returns a UUID in a Bson document. This function helps to extract the UUID.
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
/// Checks if pending order is still valid before setting `OrderStatus::Placed`.
/// Rejects order if timestamp of placement exceeds `PENDING_TIMEOUT` in relation to the order creation timestamp.
///
/// * `collection` - MongoDB collection to update.
/// * `input` - `UpdateOrderInput`.
async fn set_status_placed(collection: &Collection<Order>, id: Uuid) -> Result<()> {
    let current_timestamp_system_time = SystemTime::now();
    let order = query_object(&collection, id).await?;
    let order_created_at_system_time = order.created_at.to_system_time();
    if order_created_at_system_time + PENDING_TIMEOUT >= current_timestamp_system_time {
        let current_timestamp = DateTime::from(current_timestamp_system_time);
        set_status_placed_in_mongodb(&collection, id, current_timestamp).await
    } else {
        set_status_rejected_in_mongodb(&collection, id).await
    }
}

/// Updates order to `OrderStatus::Placed` in MongoDB.
async fn set_status_placed_in_mongodb(
    collection: &Collection<Order>,
    id: Uuid,
    current_timestamp: DateTime,
) -> Result<()> {
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

/// Updates order to `OrderStatus::Rejected` in MongoDB.
async fn set_status_rejected_in_mongodb(collection: &Collection<Order>, id: Uuid) -> Result<()> {
    let result = collection
        .update_one(
            doc! {"_id": id },
            doc! {"$set": {"order_status": OrderStatus::Rejected}},
            None,
        )
        .await;
    match result {
        Ok(_) => {
            let message = format!(
                "Order of id: `{}` was rejected as it is `OrderStatus::Pending` for too long.",
                id
            );
            return Err(Error::new(message));
        }
        Err(_) => {
            let message = format!("Order should be rejected as it is `OrderStatus::Pending` for too long. Rejecting order of id: `{}` failed in MongoDB.", id);
            return Err(Error::new(message));
        }
    }
}

/// Checks if foreign types exist (MongoDB database populated with events).
async fn validate_input(db_client: &Database, input: &CreateOrderInput) -> Result<()> {
    let user_collection: mongodb::Collection<User> = db_client.collection::<User>("users");
    validate_object(&user_collection, input.user_id).await?;
    validate_order_items(&db_client, &input.order_item_inputs).await?;
    Ok(())
}

/// Checks if all order item parameters are the system (MongoDB database populated with events).
///
/// Used before creating orders.
async fn validate_order_items(
    db_client: &Database,
    order_item_inputs: &BTreeSet<OrderItemInput>,
) -> Result<()> {
    let shipment_method_collection: mongodb::Collection<ShipmentMethod> =
        db_client.collection::<ShipmentMethod>("shipment_methods");
    let shipment_method_ids = order_item_inputs
        .iter()
        .map(|o| o.shipment_method_id)
        .collect();
    validate_objects(&shipment_method_collection, shipment_method_ids).await?;
    validate_coupons(&db_client, &order_item_inputs).await?;
    Ok(())
}

/// Checks if coupons are in the system (MongoDB database populated with events).
///
/// Used before creating orders.
async fn validate_coupons(
    db_client: &Database,
    order_item_inputs: &BTreeSet<OrderItemInput>,
) -> Result<()> {
    let coupon_collection: mongodb::Collection<Coupon> =
        db_client.collection::<Coupon>("coupons");
    let coupon_ids: Vec<Uuid> = order_item_inputs
        .iter()
        .map(|o| o.coupons.clone())
        .flatten()
        .collect();
    validate_objects(&coupon_collection, coupon_ids).await
}

/// Creates OrderItems from OrderItemInputs.
///
/// Used before creating orders.
async fn create_internal_order_items(
    db_client: &Database,
    order_item_inputs: BTreeSet<OrderItemInput>,
    current_timestamp: DateTime,
) -> Result<Vec<OrderItem>> {
    let product_variant_ids = query_product_variant_ids(&order_item_inputs).await?;
    let product_variant_versions =
        query_current_product_variant_versions(db_client, &product_variant_ids).await?;
    check_product_items_availability(&product_variant_versions).await?;
    let tax_rate_versions =
        query_current_tax_rate_versions(db_client, &product_variant_versions).await?;
    let discounts = query_discounts(&order_item_inputs, &product_variant_ids).await?;
    let shipment_fees = query_shipment_fees(&order_item_inputs, &product_variant_versions).await?;
    let internal_order_items: Vec<OrderItem> = order_item_inputs
        .iter()
        .zip(product_variant_versions)
        .zip(tax_rate_versions)
        .zip(discounts)
        .zip(shipment_fees)
        .map(
            |(
                (((order_item_input, product_variant_version), tax_rate_version), discounts),
                shipment_fee,
            )| {
                OrderItem::new(
                    order_item_input,
                    product_variant_version,
                    tax_rate_version,
                    discounts,
                    shipment_fee,
                    current_timestamp,
                )
            },
        )
        .collect();
    Ok(internal_order_items)
}

// Defines a custom scalar from GraphQL schema.
// TODO: This is hacky and i do not know which type this should be in a strongly typed language like Rust.
type _Any = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas/inventory.graphql",
    query_path = "queries/get_unreserved_product_item_counts.graphql",
    response_derives = "Debug"
)]
struct GetUnreservedProductItemCounts;

/// Checks if product items are available in the inventory service.
async fn check_product_items_availability(
    product_variant_versions: &Vec<ProductVariantVersion>,
) -> Result<()> {
    let variables = get_unreserved_product_item_counts::Variables {
        representations: vec![],
    };

    let request_body = GetUnreservedProductItemCounts::build_query(variables);

    let client = reqwest::Client::new();
    let res = client.post("/graphql").json(&request_body).send().await?;
    let response_body: Response<get_unreserved_product_item_counts::ResponseData> =
        res.json().await?;
    println!("{:#?}", response_body);
    Ok(())
}

// Defines a custom scalar from GraphQL schema.
type UUID = Uuid;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas/shoppingcart.graphql",
    query_path = "queries/get_shopping_cart_product_variant_ids.graphql",
    response_derives = "Debug"
)]
struct GetShoppingCartProductVariantIds;

/// Queries product variants from shopping cart item ids from shopping cart service.
async fn query_product_variant_ids(
    order_item_inputs: &BTreeSet<OrderItemInput>,
) -> Result<Vec<Uuid>> {
    let variables = get_shopping_cart_product_variant_ids::Variables {
        representations: vec![],
    };

    let request_body = GetShoppingCartProductVariantIds::build_query(variables);

    let client = reqwest::Client::new();
    let res = client.post("/graphql").json(&request_body).send().await?;
    let response_body: Response<get_shopping_cart_product_variant_ids::ResponseData> =
        res.json().await?;
    println!("{:#?}", response_body);
    todo!()
}

/// Obtains current product variant versions using product variants.
async fn query_current_product_variant_versions(
    db_client: &Database,
    product_variant_ids: &Vec<Uuid>,
) -> Result<Vec<ProductVariantVersion>> {
    let collection: Collection<ProductVariant> =
        db_client.collection::<ProductVariant>("product_variants");
    let product_variants = query_objects(&collection, product_variant_ids).await?;
    let current_product_variant_versions: Vec<ProductVariantVersion> =
        product_variants.iter().map(|p| p.current_version).collect();
    Ok(current_product_variant_versions)
}

/// Obtains current tax rate version for tax rate in product variant versions.
async fn query_current_tax_rate_versions(
    db_client: &Database,
    product_variant_versions: &Vec<ProductVariantVersion>,
) -> Result<Vec<TaxRateVersion>> {
    let collection: Collection<TaxRate> = db_client.collection::<TaxRate>("tax_rates");
    let tax_rate_ids: Vec<Uuid> = product_variant_versions
        .iter()
        .map(|p| p.tax_rate_id)
        .collect();
    let tax_rates = query_objects(&collection, &tax_rate_ids).await?;
    let current_tax_rate_versions: Vec<TaxRateVersion> =
        tax_rates.iter().map(|p| p.current_version).collect();
    Ok(current_tax_rate_versions)
}

// #[derive(GraphQLQuery)]
// #[graphql(
//     schema_path = "schemas/discount.graphql",
//     query_path = "queries/get_discounts.graphql",
//     response_derives = "Debug",
// )]
struct GetDiscounts;

/// Queries discounts for coupons from discount service.
async fn query_discounts(
    order_item_inputs: &BTreeSet<OrderItemInput>,
    product_variant_ids: &Vec<Uuid>,
) -> Result<Vec<HashSet<Discount>>> {
    todo!()
}

// #[derive(GraphQLQuery)]
// #[graphql(
//     schema_path = "schemas/shipment.graphql",
//     query_path = "queries/get_shipment_fees.graphql",
//     response_derives = "Debug",
// )]
struct GetShipmentFees;
/// Queries shipment fees for product variant versions and counts.
async fn query_shipment_fees(
    order_item_inputs: &BTreeSet<OrderItemInput>,
    product_variant_versions: &Vec<ProductVariantVersion>,
) -> Result<Vec<u64>> {
    todo!()
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
async fn validate_objects<T: for<'b> Deserialize<'b> + Unpin + Send + Sync + PartialEq + Clone>(
    collection: &Collection<T>,
    object_ids: Vec<Uuid>,
) -> Result<()>
where
    Uuid: From<T>,
{
    match collection
        .find(doc! {"_id": { "$in": &object_ids } }, None)
        .await
    {
        Ok(cursor) => {
            let objects: Vec<T> = cursor.try_collect().await?;
            let ids: Vec<Uuid> = objects.iter().map(|o| Uuid::from(o.clone())).collect();
            object_ids
                .iter()
                .fold(Ok(()), |o, id| match ids.contains(id) {
                    true => o.and(Ok(())),
                    false => {
                        let message = format!(
                            "{} with UUID: `{}` is not present in the system.",
                            type_name::<T>(),
                            id
                        );
                        Err(Error::new(message))
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
