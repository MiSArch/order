use async_graphql::{ComplexObject, Context, Error, Result, SimpleObject};
use bson::{doc, Document, Uuid};
use mongodb::{options::FindOptions, Collection, Database};
use mongodb_cursor_pagination::{error::CursorError, FindResult, PaginatedCursor};
use serde::{Deserialize, Serialize};

use crate::authorization::authorize_user;

use super::{
    connection::{
        base_connection::{BaseConnection, FindResultWrapper},
        order_connection::OrderConnection,
    },
    order::Order,
    order_datatypes::OrderOrderInput,
};

/// Type of a user owning orders.
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone, SimpleObject)]
#[graphql(complex)]
pub struct User {
    /// UUID of user.
    pub _id: Uuid,
    /// UUIDs of users addresses.
    #[graphql(skip)]
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
        authorize_user(&ctx, Some(self._id))?;
        let db_client = ctx.data::<Database>()?;
        let collection: Collection<Order> = db_client.collection::<Order>("orders");
        let order_order = order_by.unwrap_or_default();
        let sorting_doc = doc! {order_order.field.unwrap_or_default().as_str(): i32::from(order_order.direction.unwrap_or_default())};
        let find_options = FindOptions::builder()
            .skip(skip)
            .limit(first.map(|definitely_first| i64::from(definitely_first)))
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
