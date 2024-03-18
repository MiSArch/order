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
use std::collections::HashMap;
use std::num::TryFromIntError;
use std::time::Duration;
use std::time::SystemTime;

use crate::authentication::authenticate_user;
use crate::foreign_types::Address;
use crate::foreign_types::Coupon;
use crate::foreign_types::Discount;
use crate::foreign_types::ProductVariant;
use crate::foreign_types::ProductVariantVersion;
use crate::foreign_types::ShipmentMethod;
use crate::foreign_types::TaxRate;
use crate::foreign_types::TaxRateVersion;
use crate::mutation_input_structs::CreateOrderInput;
use crate::mutation_input_structs::OrderItemInput;
use crate::order::OrderDTO;
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
        validate_order_input(db_client, &input).await?;
        let current_timestamp = DateTime::now();
        let internal_order_items: Vec<OrderItem> =
            create_internal_order_items(&db_client, &input, current_timestamp).await?;
        let shipment_address = Address::from(input.shipment_address_id);
        let invoice_address = Address::from(input.invoice_address_id);
        let compensatable_order_amount =
            calculate_compensatable_order_amount(&internal_order_items);
        let order = Order {
            _id: Uuid::new(),
            user: User::from(input.user_id),
            created_at: current_timestamp,
            order_status: OrderStatus::Pending,
            placed_at: None,
            rejection_reason: None,
            internal_order_items,
            shipment_address,
            invoice_address,
            compensatable_order_amount,
            payment_information_id: input.payment_information_id,
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
        send_order_created_event(order).await?;
        query_order(&collection, id).await
    }
}

/// Calculates the total compensatable amount of all order items in the input by summing up their `compensatable_amount` attributes.
fn calculate_compensatable_order_amount(order_items: &Vec<OrderItem>) -> u64 {
    order_items.iter().map(|o| o.compensatable_amount).sum()
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
async fn validate_order_input(db_client: &Database, input: &CreateOrderInput) -> Result<()> {
    let user_collection: mongodb::Collection<User> = db_client.collection::<User>("users");
    validate_object(&user_collection, input.user_id).await?;
    validate_order_items(&db_client, &input.order_item_inputs).await?;
    validate_addresses(&db_client, &input).await?;
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
    let coupon_collection: mongodb::Collection<Coupon> = db_client.collection::<Coupon>("coupons");
    let coupon_ids: Vec<Uuid> = order_item_inputs
        .iter()
        .map(|o| o.coupon_ids.clone())
        .flatten()
        .collect();
    validate_objects(&coupon_collection, coupon_ids).await
}

/// Checks if addresses are registered under the user (MongoDB database populated with events).
///
/// Used before creating orders.
async fn validate_addresses(db_client: &Database, input: &CreateOrderInput) -> Result<()> {
    let user_collection: mongodb::Collection<User> = db_client.collection::<User>("users");
    validate_user_address(&user_collection, input.shipment_address_id, input.user_id).await?;
    validate_user_address(&user_collection, input.invoice_address_id, input.user_id).await
}

/// Creates OrderItems from OrderItemInputs.
///
/// Used before creating orders.
/// Each order can only contain an order item with a specific product variant once.
async fn create_internal_order_items(
    db_client: &Database,
    input: &CreateOrderInput,
    current_timestamp: DateTime,
) -> Result<Vec<OrderItem>> {
    let (counts_by_product_variant_ids, order_item_input_by_product_variant_ids) =
        query_counts_by_product_variant_ids(&input).await?;
    let product_variant_ids: Vec<Uuid> = counts_by_product_variant_ids.keys().cloned().collect();
    let product_variants_by_product_variant_ids: HashMap<Uuid, ProductVariant> =
        query_product_variants_by_product_variant_ids(db_client, &product_variant_ids).await?;
    let product_variant_versions_by_product_variant_ids =
        query_product_variant_versions_by_product_variant_ids(
            &product_variants_by_product_variant_ids,
        )
        .await;
    check_product_variant_availability(&product_variant_ids, &counts_by_product_variant_ids)
        .await?;
    let tax_rate_versions_by_product_variant_ids = query_tax_rate_versions_by_product_variant_ids(
        db_client,
        &product_variant_versions_by_product_variant_ids,
    )
    .await?;
    let order_item_input_by_product_variant_ids: HashMap<Uuid, OrderItemInput> = todo!();
    let discounts_by_product_variant_ids = query_discounts_by_product_variant_ids(
        input.user_id,
        &order_item_input_by_product_variant_ids,
        &product_variant_ids,
        &product_variant_versions_by_product_variant_ids,
        &counts_by_product_variant_ids,
    )
    .await?;
    let shipment_fees_by_product_variant_ids = query_shipment_fees_by_product_variant_ids(
        &input.order_item_inputs,
        &product_variants_by_product_variant_ids,
    )
    .await?;
    let internal_order_items = zip_to_internal_order_items(
        order_item_input_by_product_variant_ids,
        product_variants_by_product_variant_ids,
        product_variant_versions_by_product_variant_ids,
        tax_rate_versions_by_product_variant_ids,
        counts_by_product_variant_ids,
        discounts_by_product_variant_ids,
        shipment_fees_by_product_variant_ids,
        current_timestamp,
    );
    Ok(internal_order_items)
}

fn zip_to_internal_order_items(
    order_item_input_by_product_variant_ids: HashMap<Uuid, OrderItemInput>,
    product_variants_by_product_variant_ids: HashMap<Uuid, ProductVariant>,
    product_variant_versions_by_product_variant_ids: HashMap<Uuid, ProductVariantVersion>,
    tax_rate_versions_by_product_variant_ids: HashMap<Uuid, TaxRateVersion>,
    counts_by_product_variant_ids: HashMap<Uuid, u64>,
    discounts_by_product_variant_ids: HashMap<Uuid, BTreeSet<Discount>>,
    shipment_fees_by_product_variant_ids: HashMap<Uuid, u64>,
    current_timestamp: DateTime,
) -> Vec<OrderItem> {
    product_variants_by_product_variant_ids
        .iter()
        .map(|(id, product_variant)| {
            let order_item_input = order_item_input_by_product_variant_ids.get(id).unwrap();
            let product_variant_version = product_variant_versions_by_product_variant_ids
                .get(id)
                .unwrap();
            let tax_rate_version = tax_rate_versions_by_product_variant_ids.get(id).unwrap();
            let count = counts_by_product_variant_ids.get(id).unwrap();
            let internal_discounts = discounts_by_product_variant_ids.get(id).unwrap();
            let shipment_fee = shipment_fees_by_product_variant_ids.get(id).unwrap();
            OrderItem::new(
                order_item_input,
                product_variant,
                product_variant_version,
                tax_rate_version,
                *count,
                internal_discounts,
                *shipment_fee,
                current_timestamp,
            )
        })
        .collect()
}

// Defines a custom scalar from GraphQL schema.
// TODO: Check if this works somehow. This is hacky and i do not know which type this should be in a strongly typed language like Rust.
type _Any = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas_repo/inventory.graphql",
    query_path = "queries/get_unreserved_product_item_counts.graphql",
    response_derives = "Debug"
)]
struct GetUnreservedProductItemCounts;

/// Checks if product items are available in the inventory service.
async fn check_product_variant_availability(
    product_variant_ids: &Vec<Uuid>,
    counts_by_product_variant_ids: &HashMap<Uuid, u64>,
) -> Result<()> {
    let representations = product_variant_ids
        .iter()
        .cloned()
        .map(|id| id.to_string())
        .collect();
    let variables = get_unreserved_product_item_counts::Variables { representations };

    let request_body = GetUnreservedProductItemCounts::build_query(variables);
    let client = reqwest::Client::new();

    let res = client
        .post("http://localhost:3500/v1.0/invoke/inventory/method/graphql")
        .json(&request_body)
        .send()
        .await?;
    let response_body: Response<get_unreserved_product_item_counts::ResponseData> =
        res.json().await?;
    let response_data: get_unreserved_product_item_counts::ResponseData =
        response_body.data.ok_or(Error::new(
            "Response data of `check_product_variant_availability` query is empty.",
        ))?;
    let stock_counts_by_product_variant_ids =
        build_stock_counts_by_product_variant_from_response_data(response_data)?;
    calculate_availability_of_product_variant_ids(
        &stock_counts_by_product_variant_ids,
        &counts_by_product_variant_ids,
    )
}

/// Remaps the result type of the GraphQL `_entities` query retrieving stock counts for product variants.
fn build_stock_counts_by_product_variant_from_response_data(
    response_data: get_unreserved_product_item_counts::ResponseData,
) -> Result<HashMap<Uuid, u64>> {
    response_data
        .entities
        .into_iter()
        .map(|maybe_product_variant_enum| {
            let message = format!("Response data of `check_product_variant_availability` query could not be parsed, `{:?}` is `None`", maybe_product_variant_enum);
            let product_variant_enum = maybe_product_variant_enum.ok_or(Error::new(message))?;
            let stock_counts_by_product_variant: Result<(Uuid, u64)> = match product_variant_enum {
                get_unreserved_product_item_counts::GetUnreservedProductItemCountsEntities::ProductVariant(product_variant) => {
                    let message = format!("Response data of `check_product_variant_availability` query could not be parsed, `{:?}` is `None`", product_variant.product_items);
                    let product_items = product_variant.product_items.ok_or(Error::new(message))?;
                    let stock_count = u64::try_from(product_items.total_count)?;
                    Ok(
                        (
                            product_variant.id,
                            stock_count
                        )
                    )
                },
                _ => {
                    let message = format!("Response data of `check_product_variant_availability` query could not be parsed, `{:?}` is not `get_unreserved_product_item_counts::GetUnreservedProductItemCountsEntities::ProductVariant`", product_variant_enum);
                    Err(Error::new(message))
                }
            };
            stock_counts_by_product_variant
        }).collect()
}

/// Calculates the availability based on the actual and expected stock counts based on the product variant ids.
///
/// The expected amount or more product items need to be in stock for a product variant to be counted as available.
/// All product variants need to be available for this function to pass without an Err.
fn calculate_availability_of_product_variant_ids(
    stock_counts_by_product_variant_ids: &HashMap<Uuid, u64>,
    expected_stock_counts_by_product_variant_ids: &HashMap<Uuid, u64>,
) -> Result<()> {
    let availabilites: Vec<bool> = expected_stock_counts_by_product_variant_ids.iter().map(|(id, expected_count )| {
        let message = format!("Stock count for product variant of UUID: `{}` is not present in `stock_counts_by_product_variant_ids`.", id);
        let count = stock_counts_by_product_variant_ids.get(id).ok_or(Error::new(message))?;
        Ok(*count >= *expected_count)
    }).collect::<Result<Vec<bool>>>()?;
    match availabilites.into_iter().all(|b| b == true) {
        true => Ok(()),
        false => Err(Error::new(
            "Not all requested product variants are available.",
        )),
    }
}

/// Remaps the result type of the GraphQL `_entities` query retrieving unreserved product items for product variants.
fn remap_stock_counts_to_product_variants(
    discounts_for_product_variants_response_data: Vec<
        get_discounts::GetDiscountsFindApplicableDiscounts,
    >,
    product_variant_ids: &Vec<Uuid>,
) -> Result<Vec<get_discounts::GetDiscountsFindApplicableDiscounts>> {
    let mut discounts_for_product_variants: HashMap<
        Uuid,
        get_discounts::GetDiscountsFindApplicableDiscounts,
    > = discounts_for_product_variants_response_data
        .into_iter()
        .fold(
        HashMap::new(),
        |mut map: HashMap<Uuid, get_discounts::GetDiscountsFindApplicableDiscounts>,
         discount_for_product_variant: get_discounts::GetDiscountsFindApplicableDiscounts| {
            map.insert(
                discount_for_product_variant.product_variant_id,
                discount_for_product_variant,
            );
            map
        },
    );
    let graphql_client_lib_discounts: Result<Vec<get_discounts::GetDiscountsFindApplicableDiscounts>> = product_variant_ids.iter().map(|id| {
        let message = format!("Product variant of UUID: `{}` is not contained in the result which `findApplicableDiscounts` provides.", id);
        discounts_for_product_variants.remove(id).ok_or(Error::new(message))
    }).collect();
    graphql_client_lib_discounts
}

// Defines a custom scalar from GraphQL schema.
type UUID = Uuid;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas_repo/shoppingcart.graphql",
    query_path = "queries/get_shopping_cart_product_variant_ids_and_counts.graphql",
    response_derives = "Debug"
)]
struct GetShoppingCartProductVariantIdsAndCounts;

// TODO: Authentication.
/// Queries product variants from shopping cart item ids from shopping cart service.
async fn query_counts_by_product_variant_ids(
    input: &CreateOrderInput,
) -> Result<(HashMap<Uuid, u64>, HashMap<Uuid, OrderItemInput>)> {
    let representations = vec![input.user_id.to_string()];
    let variables = get_shopping_cart_product_variant_ids_and_counts::Variables { representations };

    let request_body = GetShoppingCartProductVariantIdsAndCounts::build_query(variables);
    let client = reqwest::Client::new();

    let res = client
        .post("http://localhost:3500/v1.0/invoke/shoppingcart/method/")
        .json(&request_body)
        .send()
        .await?;
    let response_body: Response<get_shopping_cart_product_variant_ids_and_counts::ResponseData> =
        res.json().await?;
    let message = "Response data of `query_counts_by_product_variant_ids` query is empty.";
    let mut response_data: get_shopping_cart_product_variant_ids_and_counts::ResponseData =
        response_body.data.ok_or(Error::new(message))?;
    let shopping_cart_response_data = response_data.entities.remove(0).ok_or(message)?;

    let ids_and_counts_by_shopping_cart_item_ids =
        into_ids_and_counts_by_shopping_cart_item_ids(shopping_cart_response_data);
    let counts_by_product_variant_ids = build_counts_by_product_variant_ids(
        &input.order_item_inputs,
        &ids_and_counts_by_shopping_cart_item_ids,
    )?;
    let order_item_inputs_by_product_variant_ids = build_order_item_inputs_by_product_variant_ids(
        &input.order_item_inputs,
        &ids_and_counts_by_shopping_cart_item_ids,
    )?;
    Ok((
        counts_by_product_variant_ids,
        order_item_inputs_by_product_variant_ids,
    ))
}

// Unwraps Enum and maps the result to a HashMap of shopping cart item ids as keys and (product_variant_id, count) as values.
fn into_ids_and_counts_by_shopping_cart_item_ids(
    ids_and_counts_enum: get_shopping_cart_product_variant_ids_and_counts::GetShoppingCartProductVariantIdsAndCountsEntities,
) -> HashMap<Uuid, (Uuid, u64)> {
    match ids_and_counts_enum {
        get_shopping_cart_product_variant_ids_and_counts::GetShoppingCartProductVariantIdsAndCountsEntities::User(user) => {
            user.shoppingcart.shoppingcart_items.nodes.iter().map(|shoppingcart_item|
                (shoppingcart_item.id, (shoppingcart_item.product_variant.id, shoppingcart_item.count as u64))
            ).collect()
        }
    }
}

/// Filters shopping cart items: `ids_and_counts` to map to `order_item_inputs`.
/// Builds HashMap which maps product variant ids to counts.
fn build_counts_by_product_variant_ids(
    order_item_inputs: &BTreeSet<OrderItemInput>,
    ids_and_counts: &HashMap<Uuid, (Uuid, u64)>,
) -> Result<HashMap<Uuid, u64>> {
    order_item_inputs
        .iter()
        .map(|e| {
            let id_and_count_ref = ids_and_counts.get(&e.shopping_cart_item_id);
            let id_and_count = id_and_count_ref.and_then(|(id, count)| Some((*id, *count)));
            id_and_count.ok_or(Error::new(
                "Shopping cart does not contain shopping cart item specified in order.",
            ))
        })
        .collect()
}

/// Filters shopping cart items: `ids_and_counts` to map to `order_item_inputs`.
/// Builds HashMap which maps product variant ids to order item inputs.
fn build_order_item_inputs_by_product_variant_ids(
    order_item_inputs: &BTreeSet<OrderItemInput>,
    ids_and_counts: &HashMap<Uuid, (Uuid, u64)>,
) -> Result<HashMap<Uuid, OrderItemInput>> {
    order_item_inputs
        .iter()
        .map(|e| {
            let id_and_count_ref = ids_and_counts.get(&e.shopping_cart_item_id);
            let id_and_count = id_and_count_ref.and_then(|(id, _)| Some((*id, e.clone())));
            id_and_count.ok_or(Error::new(
                "Shopping cart does not contain shopping cart item specified in order.",
            ))
        })
        .collect()
}

// Obtains product variants from product variant ids.
async fn query_product_variants_by_product_variant_ids(
    db_client: &Database,
    product_variant_ids: &Vec<Uuid>,
) -> Result<HashMap<Uuid, ProductVariant>> {
    let collection: Collection<ProductVariant> =
        db_client.collection::<ProductVariant>("product_variants");
    query_objects(&collection, product_variant_ids).await
}

/// Obtains current product variant versions using product variants.
async fn query_product_variant_versions_by_product_variant_ids(
    product_variants_by_product_variant_ids: &HashMap<Uuid, ProductVariant>,
) -> HashMap<Uuid, ProductVariantVersion> {
    let product_variant_versions_by_product_variant_ids: HashMap<Uuid, ProductVariantVersion> =
        product_variants_by_product_variant_ids
            .iter()
            .map(|(id, p)| (*id, p.current_version))
            .collect();
    product_variant_versions_by_product_variant_ids
}

/// Obtains current tax rate version for tax rate in product variant versions.
async fn query_tax_rate_versions_by_product_variant_ids(
    db_client: &Database,
    product_variant_versions_by_product_variant_ids: &HashMap<Uuid, ProductVariantVersion>,
) -> Result<HashMap<Uuid, TaxRateVersion>> {
    let collection: Collection<TaxRate> = db_client.collection::<TaxRate>("tax_rates");
    let tax_rate_ids: Vec<Uuid> = product_variant_versions_by_product_variant_ids
        .iter()
        .map(|(id, p)| p.tax_rate_id)
        .collect();
    let tax_rates = query_objects(&collection, &tax_rate_ids).await?;
    let tax_rate_versions_by_product_variant_ids = product_variant_versions_by_product_variant_ids.iter()
        .map(|(id, p)| {
            let message = format!("Stock count for product variant of UUID: `{}` is not present in `product_variant_versions_by_product_variant_ids`.", id);
            let tax_rate = tax_rates.get(&p.tax_rate_id).ok_or(Error::new(message))?;
            Ok((*id, tax_rate.current_version))
        })
        .collect::<Result<HashMap<Uuid, TaxRateVersion>>>()?;
    Ok(tax_rate_versions_by_product_variant_ids)
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas_repo/discount.graphql",
    query_path = "queries/get_discounts.graphql",
    response_derives = "Debug"
)]
pub struct GetDiscounts;

/// Queries discounts for coupons from discount service.
async fn query_discounts_by_product_variant_ids(
    user_id: Uuid,
    order_item_input_by_product_variant_ids: &HashMap<Uuid, OrderItemInput>,
    product_variant_ids: &Vec<Uuid>,
    product_variant_versions_by_product_variant_ids: &HashMap<Uuid, ProductVariantVersion>,
    counts_by_product_variant_ids: &HashMap<Uuid, u64>,
) -> Result<HashMap<Uuid, BTreeSet<Discount>>> {
    let find_applicable_discounts_product_variant_input =
        build_find_applicable_discounts_product_variant_input(
            order_item_input_by_product_variant_ids,
            product_variant_ids,
            counts_by_product_variant_ids,
        )?;
    let order_amount = calculate_order_amount(&product_variant_versions_by_product_variant_ids)?;
    let find_applicable_discounts_input = build_find_applicable_discounts_input(
        user_id,
        find_applicable_discounts_product_variant_input,
        order_amount,
    );
    let variables = get_discounts::Variables {
        find_applicable_discounts_input,
    };
    let request_body = GetDiscounts::build_query(variables);
    let client = reqwest::Client::new();

    let res = client
        .post("http://localhost:3500/v1.0/invoke/shoppingcart/method/")
        .json(&request_body)
        .send()
        .await?;
    let response_body: Response<get_discounts::ResponseData> = res.json().await?;
    let response_data: get_discounts::ResponseData = response_body.data.ok_or(Error::new(
        "Response data of `query_discounts` query is empty.",
    ))?;
    build_discounts_from_response_data(response_data, product_variant_ids)
}

/// Remaps the result type of the GraphQL `findApplicableDiscounts` query to the the according product variants.
/// Converts the GraphQL client library generated discounts to the internally used discounts, which are GraphQL `SimpleObject`.
fn build_discounts_from_response_data(
    response_data: get_discounts::ResponseData,
    product_variant_ids: &Vec<Uuid>,
) -> Result<HashMap<Uuid, BTreeSet<Discount>>> {
    let discounts_for_product_variants_response_data: Vec<
        get_discounts::GetDiscountsFindApplicableDiscounts,
    > = response_data.find_applicable_discounts;
    let graphql_client_lib_discounts: HashMap<
        Uuid,
        get_discounts::GetDiscountsFindApplicableDiscounts,
    > = remap_discounts_to_product_variants(
        discounts_for_product_variants_response_data,
        &product_variant_ids,
    )?;
    let simple_object_discounts = convert_graphql_client_lib_discounts_to_simple_object_discounts(
        graphql_client_lib_discounts,
    );
    Ok(simple_object_discounts)
}

/// Builds `get_discounts::FindApplicableDiscountsInput`, which is the following struct:
///
/// pub struct FindApplicableDiscountsInput {
///     #[serde(rename = "orderAmount")]
///     pub order_amount: Int,
///     #[serde(rename = "productVariants")]
///     pub product_variants: Vec<FindApplicableDiscountsProductVariantInput>,
///     #[serde(rename = "userId")]
///     pub user_id: UUID,
/// }
///
/// Describes the order amount, which is the sum of all product variant version prices, a Vec of `get_discounts::FindApplicableDiscountsProductVariantInput` and the user which the discounts are be queried for.
fn build_find_applicable_discounts_input(
    user_id: Uuid,
    find_applicable_discounts_product_variant_input: Vec<
        get_discounts::FindApplicableDiscountsProductVariantInput,
    >,
    order_amount: i64,
) -> get_discounts::FindApplicableDiscountsInput {
    let find_applicable_discounts_input = get_discounts::FindApplicableDiscountsInput {
        user_id,
        product_variants: find_applicable_discounts_product_variant_input,
        order_amount,
    };
    find_applicable_discounts_input
}

/// Builds part of the `get_discounts::FindApplicableDiscountsInput`, which is a Vec of the following struct:
///  
/// pub struct FindApplicableDiscountsProductVariantInput {
///     pub product_variant_id: Uuid,
///     pub count: u64,
///     pub coupon_ids: HashSet<Uuid>,
/// }
///
/// Describes product variant ids, the count of items planned to order and the coupons, which should be applied.
fn build_find_applicable_discounts_product_variant_input(
    order_item_input_by_product_variant_ids: &HashMap<Uuid, OrderItemInput>,
    product_variant_ids: &Vec<Uuid>,
    counts_by_product_variant_ids: &HashMap<Uuid, u64>,
) -> Result<Vec<get_discounts::FindApplicableDiscountsProductVariantInput>> {
    let find_applicable_discounts_product_variant_input: Vec<
        get_discounts::FindApplicableDiscountsProductVariantInput,
    > = product_variant_ids
        .iter()
        .map(|id| {
            let count = counts_by_product_variant_ids.get(id).unwrap();
            let coupon_ids = order_item_input_by_product_variant_ids
                .get(id)
                .unwrap()
                .coupon_ids
                .iter()
                .cloned()
                .collect();
            let find_applicable_discounts_product_variant_input =
                get_discounts::FindApplicableDiscountsProductVariantInput {
                    product_variant_id: *id,
                    count: i64::try_from(*count)?,
                    coupon_ids,
                };
            Ok::<get_discounts::FindApplicableDiscountsProductVariantInput, Error>(
                find_applicable_discounts_product_variant_input,
            )
        })
        .collect::<Result<Vec<get_discounts::FindApplicableDiscountsProductVariantInput>>>()?;
    Ok(find_applicable_discounts_product_variant_input)
}

/// Remaps the result type of the GraphQL `findApplicableDiscounts` query to the the according product variants.
fn remap_discounts_to_product_variants(
    discounts_for_product_variants_response_data: Vec<
        get_discounts::GetDiscountsFindApplicableDiscounts,
    >,
    product_variant_ids: &Vec<Uuid>,
) -> Result<HashMap<Uuid, get_discounts::GetDiscountsFindApplicableDiscounts>> {
    let mut discounts_for_product_variants: HashMap<
        Uuid,
        get_discounts::GetDiscountsFindApplicableDiscounts,
    > = discounts_for_product_variants_response_data
        .into_iter()
        .fold(
        HashMap::new(),
        |mut map: HashMap<Uuid, get_discounts::GetDiscountsFindApplicableDiscounts>,
         discount_for_product_variant: get_discounts::GetDiscountsFindApplicableDiscounts| {
            map.insert(
                discount_for_product_variant.product_variant_id,
                discount_for_product_variant,
            );
            map
        },
    );
    product_variant_ids.iter().map(|id| {
        let message = format!("Product variant of UUID: `{}` is not contained in the result which `findApplicableDiscounts` provides.", id);
        let discounts =  discounts_for_product_variants.remove(id).ok_or(Error::new(message))?;
        Ok((*id, discounts))
    }).collect()
}

/// Converts the GraphQL client library generated discounts to the internally used discounts, which are GraphQL `SimpleObject`.
///
/// This enables the discounts to be retrivable from the GraphQL endpoints of this service.
fn convert_graphql_client_lib_discounts_to_simple_object_discounts(
    graphql_client_lib_discounts: HashMap<Uuid, get_discounts::GetDiscountsFindApplicableDiscounts>,
) -> HashMap<Uuid, BTreeSet<Discount>> {
    graphql_client_lib_discounts
        .into_iter()
        .map(|(id, discounts)| {
            let discounts = discounts
                .discounts
                .into_iter()
                .map(
                    |discount: get_discounts::GetDiscountsFindApplicableDiscountsDiscounts| {
                        Discount::from(discount)
                    },
                )
                .collect();
            (id, discounts)
        })
        .collect()
}

/// Calculates the total sum of the undiscounted order items. Does not include shipping costs.
///
/// This defines the semantic of the total amount that is passed to the Discount service, for figuring out which Discounts apply.
/// Do not confuse with `calculate_compensatable_order_amount`, which is the total compensatable amount that the buyer needs to pay.
///
/// Converts value to an `i64` as this is what the GraphQL client library expects.
fn calculate_order_amount(
    pproduct_variant_versions_by_product_variant_ids: &HashMap<Uuid, ProductVariantVersion>,
) -> Result<i64, TryFromIntError> {
    let order_amount: u64 = pproduct_variant_versions_by_product_variant_ids
        .iter()
        .map(|(_, p)| p.price)
        .sum();
    i64::try_from(order_amount)
}

// #[derive(GraphQLQuery)]
// #[graphql(
//     schema_path = "schemas_repo/shipment.graphql",
//     query_path = "queries/get_shipment_fees.graphql",
//     response_derives = "Debug",
// )]
struct GetShipmentFees;
/// Queries shipment fees for product variant versions and counts.
async fn query_shipment_fees_by_product_variant_ids(
    order_item_inputs: &BTreeSet<OrderItemInput>,
    product_variants_by_product_variant_ids: &HashMap<Uuid, ProductVariant>,
) -> Result<HashMap<Uuid, u64>> {
    todo!()
}

/// Sends an `order/order/created` created event containing the order context.
async fn send_order_created_event(order: Order) -> Result<()> {
    let client = reqwest::Client::new();
    let order_dto = OrderDTO::from(order);
    client
        .post("http://localhost:3500/v1.0/publish/order/order/created")
        .json(&order_dto)
        .send()
        .await?;
    Ok(())
}

/// Checks if an address is registered under a specific user (MongoDB database populated with events).
///
/// Used before creating orders.
async fn validate_user_address(
    collection: &Collection<User>,
    id: Uuid,
    user_id: Uuid,
) -> Result<()> {
    match collection.find_one(doc! {"_id": id }, None).await {
        Ok(maybe_object) => match maybe_object {
            Some(object) => Ok(object),
            None => {
                let message = format!(
                    "Address with UUID: `{}` of user with UUID: `{}` not found.",
                    id, user_id
                );
                Err(Error::new(message))
            }
        },
        Err(_) => {
            let message = format!(
                "Address with UUID: `{}` of user with UUID: `{}` not found.",
                id, user_id
            );
            Err(Error::new(message))
        }
    }
    .map(|_| ())
}

/// Checks if a single object is in the system (MongoDB database populated with events).
///
/// Used before creating orders.
pub async fn validate_object<T: for<'a> Deserialize<'a> + Unpin + Send + Sync>(
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
