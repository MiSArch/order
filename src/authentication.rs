use async_graphql::{Context, Error, Result};
use axum::http::HeaderMap;
use bson::Uuid;
use serde::Deserialize;

// Authorized-User HTTP header.
#[derive(Deserialize, Debug)]
pub struct AuthorizedUserHeader {
    id: Uuid,
    roles: Vec<Role>,
}

// Extraction of AuthorizedUserHeader from HeaderMap.
impl TryFrom<&HeaderMap> for AuthorizedUserHeader {
    type Error = Error;

    // Tries to extract the AuthorizedUserHeader from a HeaderMap.
    //
    // Returns a GraphQL Error if the extraction fails.
    fn try_from(header_map: &HeaderMap) -> Result<Self, Self::Error> {
        if let Some(authenticate_user_header_value) = header_map.get("Authorized-User") {
            if let Ok(authenticate_user_header_str) = authenticate_user_header_value.to_str() {
                let authenticate_user_header: AuthorizedUserHeader =
                    serde_json::from_str(authenticate_user_header_str)?;
                return Ok(authenticate_user_header);
            }
        }
        Err(Error::new("Authentication failed. Authorized-User header is not set or could not be parsed."))
    }
}

// Role of user.
#[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Role {
    Buyer,
    Admin,
    Employee,
}

impl Role {
    // Defines if user has a permissive role.
    fn is_permissive(self) -> bool {
        match self {
            Self::Buyer => false,
            Self::Admin => true,
            Self::Employee => true,
        }
    }
}

// Authenticate user of UUID for a Context.
pub fn authenticate_user(ctx: &Context, id: Uuid) -> Result<()> {
    match ctx.data::<AuthorizedUserHeader>() {
        Ok(authenticate_user_header) => check_permissions(&authenticate_user_header, id),
        Err(_) => Err(Error::new("Authentication failed. Authorized-User header is not set or could not be parsed.")),
    }
}

// Check if user of UUID has a valid permission according to the AuthorizedUserHeader.
//
// Permission is valid if the user has `Role::Buyer` and the same UUID as provided in the function parameter.
// Permission is valid if the user has a permissive role: `user.is_permissive() == true`, regardless of the users UUID.
pub fn check_permissions(
    authenticate_user_header: &AuthorizedUserHeader,
    id: Uuid,
) -> Result<()> {
    if authenticate_user_header
        .roles
        .iter()
        .any(|r| r.is_permissive())
        || authenticate_user_header.id == id
    {
        return Ok(());
    } else {
        let message = format!(
            "Authentication failed for user of UUID: `{}`. Operation not permitted.",
            authenticate_user_header.id
        );
        return Err(Error::new(message));
    }
}
