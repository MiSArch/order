use async_graphql::{Context, Error, Result};
use axum::http::HeaderMap;
use bson::Uuid;
use serde::{Deserialize, Serialize};

/// `Authorized-User` HTTP header.
#[derive(Deserialize, Debug, Serialize)]
pub struct AuthorizedUserHeader {
    id: Uuid,
    roles: Vec<Role>,
}

/// Extraction of `Authorized-User` header from header map.
impl TryFrom<&HeaderMap> for AuthorizedUserHeader {
    type Error = Error;

    /// Tries to extract the `Authorized-User` header from a header map.
    ///
    /// Returns a GraphQL error if the extraction fails.
    fn try_from(header_map: &HeaderMap) -> Result<Self, Self::Error> {
        if let Some(authorized_user_header_value) = header_map.get("Authorized-User") {
            if let Ok(authorized_user_header_str) = authorized_user_header_value.to_str() {
                let authorized_user_header: AuthorizedUserHeader =
                    serde_json::from_str(authorized_user_header_str)?;
                return Ok(authorized_user_header);
            }
        }
        Err(Error::new(
            "Authorization failed. Authorized-User header is not set or could not be parsed.",
        ))
    }
}

/// Role of user.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Role {
    Buyer,
    Admin,
    Employee,
}

impl Role {
    /// Defines if user has a permissive role.
    fn is_permissive(self) -> bool {
        match self {
            Self::Buyer => false,
            Self::Admin => true,
            Self::Employee => true,
        }
    }
}

/// Authorize user of UUID for a context.
///
/// * `context` - GraphQL context containing the `Authorized-User` header.
/// * `id` - Option of UUID of the user to authorize.
pub fn authorize_user(ctx: &Context, id: Option<Uuid>) -> Result<()> {
    match ctx.data::<AuthorizedUserHeader>() {
        Ok(authorized_user_header) => check_permissions(&authorized_user_header, id),
        Err(_) => Err(Error::new(
            "Authentication failed. Authorized-User header is not set or could not be parsed.",
        )),
    }
}

/// Check if user of UUID has a valid permission according to the `Authorized-User` header.
///
/// Permission is valid if the user has `Role::Buyer` and the same UUID as provided in the function parameter.
/// Permission is valid if the user has a permissive role: `user.is_permissive() == true`, regardless of the users UUID.
///
/// * `authorized_user_header` - `Authorized-User` header containing the users UUID and role.
/// * `id` - Option of UUID of the user to authorize.
pub fn check_permissions(
    authorized_user_header: &AuthorizedUserHeader,
    id: Option<Uuid>,
) -> Result<()> {
    let id_contained_in_header = id
        .and_then(|id| Some(authorized_user_header.id == id))
        .unwrap_or(false);
    if authorized_user_header
        .roles
        .iter()
        .any(|role| role.is_permissive())
        || id_contained_in_header
    {
        return Ok(());
    } else {
        let message = format!(
            "Authentication failed for user of UUID: `{}`. Operation not permitted.",
            authorized_user_header.id
        );
        return Err(Error::new(message));
    }
}
