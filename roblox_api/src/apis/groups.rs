use std::{borrow::Borrow, fmt::Display};

use crate::{AuthenticatedClient, BaseClient, Empty, Id, RequestResult};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::Itertools;
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Shout {
    pub body: String,
    pub poster: DetailedOwner,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}
#[derive(Deserialize, Debug)]
pub struct BatchOwner {
    pub id: Id,
    #[serde(flatten)]
    pub r#type: OwnerType,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum OwnerType {
    User,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DetailedOwner {
    pub has_verified_badge: bool,
    pub user_id: Id,
    pub username: String,
    pub display_name: String,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BatchInfo {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub owner: Option<BatchOwner>,
    pub created: DateTime<Utc>,
    pub has_verified_badge: bool,
}
#[derive(Deserialize, Debug)]
struct BatchResponse {
    data: Vec<BatchInfo>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct DetailedInfo {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub owner: Option<DetailedOwner>,
    pub shout: Option<Shout>,
    pub member_count: u64,
    pub is_builders_club_only: bool,
    pub public_entry_allowed: bool,
    pub has_verified_badge: bool,
    #[serde(default)]
    pub is_locked: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SolvedCaptcha<'a> {
    pub session_id: &'a str,
    pub redemption_token: &'a str,
    pub captcha_id: &'a str,
    pub captcha_token: &'a str,
    pub captcha_provider: &'a str,
    pub challenge_id: &'a str,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct Metadata {
    pub group_limit: u16,
    pub current_group_count: u16,
    pub group_status_max_length: u16,
    pub group_post_max_length: u16,

    #[serde(rename = "isGroupWallNotificationsEnabled")]
    pub group_wall_notifications_enabled: bool,

    #[serde(rename = "groupWallNotificationsSubscribeIntervalInMilliseconds")]
    pub group_wall_notifications_subscribe_interval: u32,

    #[serde(rename = "areProfileGroupsHidden")]
    pub profile_groups_hidden: bool,

    #[serde(rename = "isGroupDetailsPolicyEnabled")]
    pub group_details_policy_enabled: bool,
    pub show_previous_group_names: bool,
}

macro_rules! add_base_url {
    ($api_route: literal) => {
        concat!("https://groups.roblox.com/", $api_route)
    };
    ($api_format_string: literal, $($args:expr),+) => {
        &format!(concat!("https://groups.roblox.com/", $api_format_string), $($args),+)
    };
}
#[async_trait]
pub trait GroupsApi: BaseClient {
    async fn get_batch_info(
        &self,
        mut group_ids: impl Iterator<Item = impl Borrow<Id> + Display> + Send,
    ) -> RequestResult<Vec<BatchInfo>> {
        let mut api_url = Url::parse(add_base_url!("v2/groups")).unwrap();
        api_url.set_query(Some(&format!("groupIds={}", group_ids.join(","))));
        let response = self.get::<BatchResponse>(api_url, None).await?;
        Ok(response.data)
    }
    async fn get_detailed_info(&self, group: Id) -> RequestResult<DetailedInfo> {
        let response = self
            .get::<DetailedInfo>(add_base_url!("v1/groups/{}", group), None)
            .await?;
        Ok(response)
    }
    async fn get_metadata(&self) -> RequestResult<Metadata> {
        let response = self
            .get::<Metadata>(add_base_url!("v1/groups/metadata"), None)
            .await?;
        Ok(response)
    }
}
impl<T: BaseClient> GroupsApi for T {}

#[async_trait]
pub trait GroupsAuthenticatedApi: AuthenticatedClient {
    async fn join_group<'a>(
        &self,
        group: Id,
        solved_captcha: impl Into<Option<SolvedCaptcha<'a>>> + Send,
    ) -> RequestResult<Empty> {
        let response = self
            .authenticated_post::<Empty, SolvedCaptcha<'a>>(
                add_base_url!("v1/groups/{}/users", group),
                solved_captcha,
            )
            .await?;
        Ok(response)
    }
    async fn claim_group(&self, group: Id) -> RequestResult<Empty> {
        let response = self
            .authenticated_post::<Empty, ()>(
                add_base_url!("v1/groups/{}/claim-ownership", group),
                None,
            )
            .await?;
        Ok(response)
    }
    async fn remove_user_from_group(&self, group: Id, target: Id) -> RequestResult<Empty> {
        let response = self
            .authenticated_delete::<Empty, ()>(
                add_base_url!("v1/groups/{}/users/{}", group, target),
                None,
            )
            .await?;
        Ok(response)
    }
}
impl<T: AuthenticatedClient> GroupsAuthenticatedApi for T {}
