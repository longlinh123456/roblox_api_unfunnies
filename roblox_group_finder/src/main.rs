#![deny(
    clippy::all
    clippy::pedantic,
    clippy::nursery
)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::unreadable_literal
)]

mod config;
mod constants;
mod init;
mod status_display;
mod threads;
mod utils;

use anyhow::{bail, Context, Result};
use config::Settings;
use indicatif::{ProgressBar, ProgressStyle};
use once_cell::sync::OnceCell;
use roblox_api::{
    apis::{groups::GroupsApi, users::UsersAuthenticatedApi},
    clients::{ClientBuilder, CookieClient},
};
use tokio::task;
use tracing::info;

use utils::GroupsApiExt;

use crate::{constants::BROWSER_ID_COOKIE_NAME, status_display::LogWriter};

static SETTINGS: OnceCell<Settings> = OnceCell::new();

#[tokio::main]
#[allow(clippy::cast_possible_truncation, unused_must_use)]
async fn main() -> Result<()> {
    let config = config::get_config()?;
    SETTINGS.set(config.settings).unwrap();
    let settings = SETTINGS.get().unwrap();

    if config.proxies.is_empty() {
        bail!("No proxies provided");
    };
    let auth_client = CookieClient::new(
        ClientBuilder::new().no_proxy().http2_prior_knowledge(),
        &settings.cookie,
    );
    auth_client.insert_cookie(BROWSER_ID_COOKIE_NAME, &settings.browser_id);

    let metadata = auth_client
        .get_metadata()
        .await
        .with_context(|| "Failed to get group metadata")?;
    if metadata.group_limit == 0 {
        bail!("Auth cookie provided is invalid");
    }
    let user_id = auth_client
        .get_authenticated_user()
        .await
        .with_context(|| "Failed to get account's user ID")?
        .id;

    let bar = ProgressBar::new(0).with_style(ProgressStyle::with_template("{msg}").unwrap());
    let cloned_bar = bar.clone();
    tracing_subscriber::fmt()
        .with_writer(move || LogWriter::new(cloned_bar.clone()))
        .init();

    let claim_receiver = init::init_check_threads(
        auth_client
            .get_latest_group_id()
            .await
            .with_context(|| "Failed to get latest group id")?
            .get() as usize,
        &bar,
        &config.proxies,
        metadata.group_limit,
    );

    info!("Starting claim task");
    task::spawn(threads::claim(
        auth_client,
        claim_receiver.to_async(),
        metadata,
        user_id,
    ))
    .await;

    Ok(())
}
