use async_graphql::{ComplexObject, Context, Error, Result, SimpleObject};
use bson::{doc, Document, Uuid};
use mongodb::{options::FindOptions, Collection, Database};
use mongodb_cursor_pagination::{error::CursorError, FindResult, PaginatedCursor};
use serde::{Deserialize, Serialize};

use crate::{
    authentication::authenticate_user,
    base_connection::{BaseConnection, FindResultWrapper},
    order::Order,
    order_connection::OrderConnection,
    order_datatypes::OrderOrderInput,
};

/// Type of a user owning orders.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone, SimpleObject)]
#[graphql(complex)]
pub struct User {
    /// UUID of the user.
    pub _id: Uuid,
    /// UUIDs of the users addresses.
    pub user_address_ids: Vec<Uuid>,
}

#[ComplexObject]
impl User {
    /// Retrieves orders of user.
    async fn orders<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "Describes that the `first` N orders should be retrieved.")] first: Option<
            u32,
        >,
        #[graphql(desc = "Describes how many orders should be skipped at the beginning.")]
        skip: Option<u64>,
        #[graphql(desc = "Specifies the order in which orders are retrieved.")] order_by: Option<
            OrderOrderInput,
        >,
    ) -> Result<OrderConnection> {
        authenticate_user(&ctx, self._id)?;
        let db_client = ctx.data::<Database>()?;
        let collection: Collection<Order> = db_client.collection::<Order>("orders");
        let order_order = order_by.unwrap_or_default();
        let sorting_doc = doc! {order_order.field.unwrap_or_default().as_str(): i32::from(order_order.direction.unwrap_or_default())};
        let find_options = FindOptions::builder()
            .skip(skip)
            .limit(first.map(|v| i64::from(v)))
            .sort(sorting_doc)
            .build();
        let document_collection = collection.clone_with_type::<Document>();
        let filter = doc! {"user._id": self._id};
        let maybe_find_results: Result<FindResult<Order>, CursorError> =
            PaginatedCursor::new(Some(find_options.clone()), None, None)
                .find(&document_collection, Some(&filter))
                .await;
        match maybe_find_results {
            Ok(find_results) => {
                let find_result_wrapper = FindResultWrapper(find_results);
                let connection = Into::<BaseConnection<Order>>::into(find_result_wrapper);
                Ok(Into::<OrderConnection>::into(connection))
            }
            Err(_) => return Err(Error::new("Retrieving orders failed in MongoDB.")),
        }
    }
}

impl From<Uuid> for User {
    fn from(value: Uuid) -> Self {
        User {
            _id: value,
            user_address_ids: vec![],
        }
    }
}
