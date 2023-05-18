use async_trait::async_trait;
use serde::Deserialize;

use crate::{AuthenticatedClient, Id, RequestResult};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Robux {
    robux: u64,
}

macro_rules! add_base_url {
    ($api_route: literal) => {
        concat!("https://economy.roblox.com/", $api_route)
    };
    ($api_format_string: literal, $($args:expr),+) => {
        &format!(concat!("https://economy.roblox.com/", $api_format_string), $($args),+)
    };
}

#[async_trait]
pub trait EconomyAuthenticatedApi: AuthenticatedClient {
    async fn get_group_funds(&self, group: Id) -> RequestResult<u64> {
        let response = self
            .authenticated_get::<Robux>(add_base_url!("v1/groups/{}/currency", group), None)
            .await?;
        Ok(response.robux)
    }
}
impl<T: AuthenticatedClient> EconomyAuthenticatedApi for T {}
