use std::{env, fs::File, io::Write};

use async_graphql::{
    extensions::Logger, http::GraphiQLSource, EmptySubscription, SDLExportOptions, Schema,
};

use async_graphql_axum::{GraphQLRequest, GraphQLResponse};

use axum::{
    extract::State,
    http::header::HeaderMap,
    response::{self, IntoResponse},
    routing::{get, post},
    Router, Server,
};

use clap::{arg, command, Parser};

use order_compensation::OrderCompensation;
use simple_logger::SimpleLogger;

use log::info;
use mongodb::{options::ClientOptions, Client, Database};

mod order;
use order::Order;

mod order_item;

mod query;
use query::Query;

mod mutation;
use mutation::Mutation;

use foreign_types::{Coupon, ProductVariant, ProductVariantVersion, ShipmentMethod, TaxRate};

mod user;
use user::User;

mod http_event_service;
use http_event_service::{
    list_topic_subscriptions, on_id_creation_event, on_product_variant_version_creation_event,
    on_shipment_creation_failed_event, on_tax_rate_version_creation_event,
    on_user_address_archived_event, on_user_address_creation_event, HttpEventServiceState,
};

mod authentication;
use authentication::AuthorizedUserHeader;

mod order_compensation;

mod base_connection;
mod discount_connection;
mod foreign_types;
mod mutation_input_structs;
mod order_connection;
mod order_datatypes;
mod order_item_connection;
mod product_variant_version_connection;

/// Builds the GraphiQL frontend.
async fn graphiql() -> impl IntoResponse {
    response::Html(GraphiQLSource::build().endpoint("/").finish())
}

/// Establishes database connection and returns the client.
async fn db_connection() -> Client {
    let uri = match env::var_os("MONGODB_URI") {
        Some(uri) => uri.into_string().unwrap(),
        None => panic!("$MONGODB_URI is not set."),
    };

    // Parse a connection string into an options struct.
    let mut client_options = ClientOptions::parse(uri).await.unwrap();

    // Manually set an option.
    client_options.app_name = Some("Order".to_string());

    // Get a handle to the deployment.
    Client::with_options(client_options).unwrap()
}

/// Returns Router that establishes connection to Dapr.
///
/// Creates endpoints to define pub/sub interaction with Dapr.
async fn build_dapr_router(db_client: Database) -> Router {
    let product_variant_collection: mongodb::Collection<ProductVariant> =
        db_client.collection::<ProductVariant>("product_variants");
    let product_variant_version_collection: mongodb::Collection<ProductVariantVersion> =
        db_client.collection::<ProductVariantVersion>("product_variant_versions");
    let coupon_collection: mongodb::Collection<Coupon> = db_client.collection::<Coupon>("coupons");
    let tax_rate_collection: mongodb::Collection<TaxRate> =
        db_client.collection::<TaxRate>("tax_rates");
    let shipment_method_collection: mongodb::Collection<ShipmentMethod> =
        db_client.collection::<ShipmentMethod>("shipment_methods");
    let user_collection: mongodb::Collection<User> = db_client.collection::<User>("users");
    let order_collection: mongodb::Collection<Order> = db_client.collection::<Order>("orders");
    let order_compensation_collection: mongodb::Collection<OrderCompensation> =
        db_client.collection::<OrderCompensation>("order_compensations");

    // Define routes.
    let app = Router::new()
        .route("/dapr/subscribe", get(list_topic_subscriptions))
        .route("/on-id-creation-event", post(on_id_creation_event))
        .route(
            "/on-product-variant-version-creation-event",
            post(on_product_variant_version_creation_event),
        )
        .route(
            "/on-tax-rate-version-creation-event",
            post(on_tax_rate_version_creation_event),
        )
        .route(
            "/on-user-address-creation-event",
            post(on_user_address_creation_event),
        )
        .route(
            "/on-user-address-archived-event",
            post(on_user_address_archived_event),
        )
        .route(
            "/on-shipment-creation-failed-event",
            post(on_shipment_creation_failed_event),
        )
        .with_state(HttpEventServiceState {
            product_variant_collection,
            product_variant_version_collection,
            coupon_collection,
            tax_rate_collection,
            shipment_method_collection,
            user_collection,
            order_collection,
            order_compensation_collection,
        });
    app
}

/// Command line argument to toggle schema generation instead of service execution.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Generates GraphQL schema in `./schemas/order.graphql`.
    #[arg(long)]
    generate_schema: bool,
}

/// Activates logger and parses argument for optional schema generation. Otherwise starts gRPC and GraphQL server.
#[tokio::main]
async fn main() -> std::io::Result<()> {
    SimpleLogger::new().init().unwrap();

    let args = Args::parse();
    if args.generate_schema {
        let schema = Schema::build(Query, Mutation, EmptySubscription).finish();
        let mut file = File::create("./schemas/order.graphql")?;
        let sdl_export_options = SDLExportOptions::new().federation();
        let schema_sdl = schema.sdl_with_options(sdl_export_options);
        file.write_all(schema_sdl.as_bytes())?;
        info!("GraphQL schema: ./schemas/order.graphql was successfully generated!");
    } else {
        start_service().await;
    }
    Ok(())
}

/// Describes the handler for GraphQL requests.
///
/// Parses the "Authenticate-User" header and writes it in the context data of the specfic request.
/// Then executes the GraphQL schema with the request.
async fn graphql_handler(
    State(schema): State<Schema<Query, Mutation, EmptySubscription>>,
    headers: HeaderMap,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let mut req = req.into_inner();
    if let Ok(authenticate_user_header) = AuthorizedUserHeader::try_from(&headers) {
        req = req.data(authenticate_user_header);
    }
    schema.execute(req).await.into()
}

/// Starts order service on port 8000.
async fn start_service() {
    let client = db_connection().await;
    let db_client: Database = client.database("order-database");

    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .extension(Logger)
        .data(db_client.clone())
        .enable_federation()
        .finish();

    let graphiql = Router::new()
        .route("/", get(graphiql).post(graphql_handler))
        .with_state(schema);
    let dapr_router = build_dapr_router(db_client).await;
    let app = Router::new().merge(graphiql).merge(dapr_router);

    info!("GraphiQL IDE: http://0.0.0.0:8080");
    Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
