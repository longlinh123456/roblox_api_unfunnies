use async_trait::async_trait;
use serde::Deserialize;

use crate::{AuthenticatedClient, Id, RequestResult};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatedUser {
    pub id: Id,
    pub name: String,
    pub display_name: String,
}
macro_rules! add_base_url {
    ($api_route: literal) => {
        concat!("https://users.roblox.com/", $api_route)
    };
    ($api_format_string: literal, $($args:expr),+) => {
        &format!(concat!("https://users.roblox.com/", $api_format_string), $($args),+)
    };
}

#[async_trait]
pub trait UsersAuthenticatedApi: AuthenticatedClient {
    async fn get_authenticated_user(&self) -> RequestResult<AuthenticatedUser> {
        let response = self
            .authenticated_get::<AuthenticatedUser>(add_base_url!("v1/users/authenticated"), None)
            .await?;
        Ok(response)
    }
}
impl<T: AuthenticatedClient> UsersAuthenticatedApi for T {}
