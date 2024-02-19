use crate::{authentication::authenticate_user, order_item::OrderItem, user::User, Order};
use async_graphql::{Context, Error, Object, Result};

use bson::Uuid;
use mongodb::{bson::doc, Collection, Database};

/// Describes GraphQL order queries.
pub struct Query;

#[Object]
impl Query {
    /// Entity resolver for user of specific id.
    #[graphql(entity)]
    async fn user_entity_resolver<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "UUID of user to retrieve.")] id: Uuid,
    ) -> Result<User> {
        let db_client = ctx.data_unchecked::<Database>();
        let collection: Collection<User> = db_client.collection::<User>("users");
        query_user(&collection, id).await
    }

    /// Retrieves order of specific id.
    async fn order<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "UUID of order to retrieve.")] id: Uuid,
    ) -> Result<Order> {
        let db_client = ctx.data_unchecked::<Database>();
        let collection: Collection<Order> = db_client.collection::<Order>("orders");
        let order = query_order(&collection, id).await?;
        authenticate_user(&ctx, order.user._id)?;
        Ok(order)
    }

    /// Entity resolver for order of specific id.
    #[graphql(entity)]
    async fn order_entity_resolver<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(key, desc = "UUID of order to retrieve.")] id: Uuid,
    ) -> Result<Order> {
        let db_client = ctx.data_unchecked::<Database>();
        let collection: Collection<Order> = db_client.collection::<Order>("orders");
        let order = query_order(&collection, id).await?;
        authenticate_user(&ctx, order.user._id)?;
        Ok(order)
    }

    /// Retrieves order_item of specific id.
    async fn order_item<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "UUID of order_item to retrieve.")] id: Uuid,
    ) -> Result<OrderItem> {
        let db_client = ctx.data_unchecked::<Database>();
        let collection: Collection<OrderItem> = db_client.collection::<OrderItem>("order_items");
        let order_item = query_order_item(&collection, id).await?;
        // TODO: Authentication
        //authenticate_user(&ctx, order_item.user._id)?;
        Ok(order_item)
    }

    /// Entity resolver for order_item of specific id.
    #[graphql(entity)]
    async fn order_item_entity_resolver<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(key, desc = "UUID of order_item to retrieve.")] id: Uuid,
    ) -> Result<OrderItem> {
        let db_client = ctx.data_unchecked::<Database>();
        let collection: Collection<OrderItem> = db_client.collection::<OrderItem>("order_items");
        let order_item = query_order_item(&collection, id).await?;
        // TODO: Authentication
        //authenticate_user(&ctx, order_item.user._id)?;
        Ok(order_item)
    }
}

/// Shared function to query a order from a MongoDB collection of orders
///
/// * `connection` - MongoDB database connection.
/// * `id` - UUID of order.
pub async fn query_order(collection: &Collection<Order>, id: Uuid) -> Result<Order> {
    match collection.find_one(doc! {"_id": id }, None).await {
        Ok(maybe_order) => match maybe_order {
            Some(order) => Ok(order),
            None => {
                let message = format!("Order with UUID id: `{}` not found.", id);
                Err(Error::new(message))
            }
        },
        Err(_) => {
            let message = format!("Order with UUID id: `{}` not found.", id);
            Err(Error::new(message))
        }
    }
}

/// Shared function to query an order item from a MongoDB collection of order items.
///
/// * `connection` - MongoDB database connection.
/// * `id` - UUID of order_item.
pub async fn query_order_item(collection: &Collection<OrderItem>, id: Uuid) -> Result<OrderItem> {
    match collection.find_one(doc! {"_id": id }, None).await {
        Ok(maybe_order_item) => match maybe_order_item {
            Some(order_item) => Ok(order_item),
            None => {
                let message = format!("OrderItem with UUID id: `{}` not found.", id);
                Err(Error::new(message))
            }
        },
        Err(_) => {
            let message = format!("OrderItem with UUID id: `{}` not found.", id);
            Err(Error::new(message))
        }
    }
}

/// Shared function to query a user from a MongoDB collection of users.
///
/// * `connection` - MongoDB database connection.
/// * `id` - UUID of user.
pub async fn query_user(collection: &Collection<User>, id: Uuid) -> Result<User> {
    match collection.find_one(doc! {"_id": id }, None).await {
        Ok(maybe_user) => match maybe_user {
            Some(user) => Ok(user),
            None => {
                let message = format!("User with UUID id: `{}` not found.", id);
                Err(Error::new(message))
            }
        },
        Err(_) => {
            let message = format!("User with UUID id: `{}` not found.", id);
            Err(Error::new(message))
        }
    }
}
