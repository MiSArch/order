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
        let total_compensatable_amount =
            calculate_total_compensatable_amount(&internal_order_items);
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
            total_compensatable_amount,
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
fn calculate_total_compensatable_amount(order_items: &Vec<OrderItem>) -> u64 {
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
        .map(|o| o.coupons.clone())
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
async fn create_internal_order_items(
    db_client: &Database,
    input: &CreateOrderInput,
    current_timestamp: DateTime,
) -> Result<Vec<OrderItem>> {
    let (product_variant_ids, counts) = query_product_variant_ids_and_counts(&input).await?;
    let product_variants: Vec<ProductVariant> =
        query_product_variants(db_client, &product_variant_ids).await?;
    let product_variant_versions =
        query_current_product_variant_versions(&product_variants).await?;
    check_product_variant_availability(&product_variant_ids, &counts).await?;
    let tax_rate_versions =
        query_current_tax_rate_versions(db_client, &product_variant_versions).await?;
    let discounts = query_discounts(
        &input,
        &product_variant_ids,
        &product_variant_versions,
        &counts,
    )
    .await?;
    let shipment_fees =
        query_shipment_fees(&input.order_item_inputs, &product_variant_versions).await?;
    let internal_order_items = zip_to_internal_order_items(
        input,
        product_variants,
        product_variant_versions,
        tax_rate_versions,
        counts,
        discounts,
        shipment_fees,
        current_timestamp,
    );
    Ok(internal_order_items)
}

/// Takes all retrieved values needed for creating the internal order items and zip them to create the according order items.
fn zip_to_internal_order_items(
    input: &CreateOrderInput,
    product_variants: Vec<ProductVariant>,
    product_variant_versions: Vec<ProductVariantVersion>,
    tax_rate_versions: Vec<TaxRateVersion>,
    counts: Vec<u64>,
    discounts: Vec<BTreeSet<Discount>>,
    shipment_fees: Vec<u64>,
    current_timestamp: DateTime,
) -> Vec<OrderItem> {
    let internal_order_items: Vec<OrderItem> = input
        .order_item_inputs
        .iter()
        .zip(product_variants)
        .zip(product_variant_versions)
        .zip(tax_rate_versions)
        .zip(counts)
        .zip(discounts)
        .zip(shipment_fees)
        .map(
            |(
                (
                    (
                        (
                            ((order_item_input, product_variant), product_variant_version),
                            tax_rate_version,
                        ),
                        count,
                    ),
                    discounts,
                ),
                shipment_fee,
            )| {
                OrderItem::new(
                    order_item_input,
                    product_variant,
                    product_variant_version,
                    tax_rate_version,
                    count,
                    discounts,
                    shipment_fee,
                    current_timestamp,
                )
            },
        )
        .collect();
    internal_order_items
}

// Defines a custom scalar from GraphQL schema.
// TODO: This is hacky and i do not know which type this should be in a strongly typed language like Rust.
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
    counts: &Vec<u64>,
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
    calculate_availability_from_response_data_and_counts(response_data, counts)
}

/// Calculates the availability by checking if all elements in the reponse data are available.
fn calculate_availability_from_response_data_and_counts(
    response_data: get_unreserved_product_item_counts::ResponseData,
    counts: &Vec<u64>,
) -> Result<()> {
    match response_data
        .entities
        .into_iter()
        .zip(counts)
        .all(product_variant_is_available)
    {
        true => Ok(()),
        false => Err(Error::new(
            "Not all product variants associated with order items in order are available.",
        )),
    }
}

/// Unwraps an Option of `get_unreserved_product_item_counts::GetUnreservedProductItemCountsEntities` to check availability.
/// Available is defined as the amount of items to reserve (`count) greater or equal than the amount of items in stock (`product_items.total_count`).
///
/// Assumes that all options are `Some` and `product_items.total_count` is non-negative, if not returns `false`.
fn product_variant_is_available(
    (maybe_product_variant_enum, count): (
        Option<get_unreserved_product_item_counts::GetUnreservedProductItemCountsEntities>,
        &u64,
    ),
) -> bool {
    let maybe_availability = maybe_product_variant_enum.and_then(|product_variant_enum: get_unreserved_product_item_counts::GetUnreservedProductItemCountsEntities| {
        match product_variant_enum {
            get_unreserved_product_item_counts::GetUnreservedProductItemCountsEntities::ProductVariant(product_variant) => {
                product_variant.product_items.and_then(|product_items|
                    u64::try_from(product_items.total_count).and_then(|product_items_total_count| Ok(product_items_total_count >= *count)).ok()
                )
            },
        }
    });
    maybe_availability.unwrap_or(false)
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
async fn query_product_variant_ids_and_counts(
    input: &CreateOrderInput,
) -> Result<(Vec<Uuid>, Vec<u64>)> {
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
    let message = "Response data of `query_product_variant_ids_and_counts` query is empty.";
    let mut response_data: get_shopping_cart_product_variant_ids_and_counts::ResponseData =
        response_body.data.ok_or(Error::new(message))?;
    let shopping_cart_response_data = response_data.entities.remove(0).ok_or(message)?;

    let ids_and_counts_by_shopping_cart_item_ids =
        into_ids_and_counts_by_shopping_cart_item_ids(shopping_cart_response_data);
    let ids_and_counts = map_order_item_input_to_ids_and_counts(
        &input.order_item_inputs,
        &ids_and_counts_by_shopping_cart_item_ids,
    )?;
    let ids = ids_and_counts.iter().map(|(id, _)| *id).collect();
    let counts = ids_and_counts.iter().map(|(_, count)| *count).collect();
    Ok((ids, counts))
}

// Unwraps Enum and maps the result to a hash map of shopping cart item ids as keys and (product_variant_id, count) as values.
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

/// Maps order item input to queried ids and counts.
fn map_order_item_input_to_ids_and_counts(
    order_item_inputs: &BTreeSet<OrderItemInput>,
    ids_and_counts: &HashMap<Uuid, (Uuid, u64)>,
) -> Result<Vec<(Uuid, u64)>> {
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

// Obtains product variants from product variant ids.
async fn query_product_variants(
    db_client: &Database,
    product_variant_ids: &Vec<Uuid>,
) -> Result<Vec<ProductVariant>> {
    let collection: Collection<ProductVariant> =
        db_client.collection::<ProductVariant>("product_variants");
    query_objects(&collection, product_variant_ids).await
}

/// Obtains current product variant versions using product variants.
async fn query_current_product_variant_versions(
    product_variants: &Vec<ProductVariant>,
) -> Result<Vec<ProductVariantVersion>> {
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

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas_repo/discount.graphql",
    query_path = "queries/get_discounts.graphql",
    response_derives = "Debug"
)]
pub struct GetDiscounts;

/// Queries discounts for coupons from discount service.
async fn query_discounts(
    input: &CreateOrderInput,
    product_variant_ids: &Vec<Uuid>,
    product_variant_versions: &Vec<ProductVariantVersion>,
    counts: &Vec<u64>,
) -> Result<Vec<BTreeSet<Discount>>> {
    let find_applicable_discounts_product_variant_input =
        build_find_applicable_discounts_product_variant_input(input, product_variant_ids, counts)?;
    let order_amount = calculate_order_amount(&product_variant_versions)?;
    let find_applicable_discounts_input = build_find_applicable_discounts_input(
        input,
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
) -> Result<Vec<BTreeSet<Discount>>> {
    let discounts_for_product_variants_response_data: Vec<
        get_discounts::GetDiscountsFindApplicableDiscounts,
    > = response_data.find_applicable_discounts;
    let graphql_client_lib_discounts: Vec<get_discounts::GetDiscountsFindApplicableDiscounts> =
        remap_discounts_to_product_variants(
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
    input: &CreateOrderInput,
    find_applicable_discounts_product_variant_input: Vec<
        get_discounts::FindApplicableDiscountsProductVariantInput,
    >,
    order_amount: i64,
) -> get_discounts::FindApplicableDiscountsInput {
    let find_applicable_discounts_input = get_discounts::FindApplicableDiscountsInput {
        user_id: input.user_id,
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
    input: &CreateOrderInput,
    product_variant_ids: &Vec<Uuid>,
    counts: &Vec<u64>,
) -> Result<Vec<get_discounts::FindApplicableDiscountsProductVariantInput>> {
    let find_applicable_discounts_product_variant_input: Vec<
        get_discounts::FindApplicableDiscountsProductVariantInput,
    > = input
        .order_item_inputs
        .iter()
        .zip(product_variant_ids)
        .zip(counts)
        .map(|((order_item_input, product_variant_id), count)| {
            let find_applicable_discounts_product_variant_input =
                get_discounts::FindApplicableDiscountsProductVariantInput {
                    product_variant_id: *product_variant_id,
                    count: i64::try_from(*count)?,
                    coupon_ids: order_item_input.coupons.iter().cloned().collect(),
                };
            Ok::<get_discounts::FindApplicableDiscountsProductVariantInput, Error>(
                find_applicable_discounts_product_variant_input,
            )
        })
        .collect::<Result<Vec<get_discounts::FindApplicableDiscountsProductVariantInput>>>()?;
    Ok(find_applicable_discounts_product_variant_input)
}

/// Remaps the result type of the GraphQL `findApplicableDiscounts` query to the the according product variants.
///
/// Builds a discounts Vec which is ordered identically to the `product_variant_ids` Vec.
///
/// This is needed to reconstruct the order from the GraphQL query output, which does not rely on correct ordering.
fn remap_discounts_to_product_variants(
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

/// Converts the GraphQL client library generated discounts to the internally used discounts, which are GraphQL `SimpleObject`.
///
/// This enables the discounts to be retrivable from the GraphQL endpoints of this service.
fn convert_graphql_client_lib_discounts_to_simple_object_discounts(
    graphql_client_lib_discounts: Vec<get_discounts::GetDiscountsFindApplicableDiscounts>,
) -> Vec<BTreeSet<Discount>> {
    graphql_client_lib_discounts
        .into_iter()
        .map(
            |discounts: get_discounts::GetDiscountsFindApplicableDiscounts| {
                discounts
                    .discounts
                    .into_iter()
                    .map(
                        |discount: get_discounts::GetDiscountsFindApplicableDiscountsDiscounts| {
                            Discount::from(discount)
                        },
                    )
                    .collect()
            },
        )
        .collect()
}

/// Calculates the total sum of the undiscounted order items. Does not include shipping costs.
///
/// This defines the semantic of the total amount that is passed to the Discount service, for figuring out which Discounts apply.
/// Do not confuse with `calculate_total_compensatable_amount`, which is the total compensatable amount that the buyer needs to pay.
///
/// Converts value to an `i64` as this is what the GraphQL client library expects.
fn calculate_order_amount(
    product_variant_versions: &Vec<ProductVariantVersion>,
) -> Result<i64, TryFromIntError> {
    let order_amount: u64 = product_variant_versions.iter().map(|p| p.price).sum();
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
async fn query_shipment_fees(
    order_item_inputs: &BTreeSet<OrderItemInput>,
    product_variant_versions: &Vec<ProductVariantVersion>,
) -> Result<Vec<u64>> {
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
