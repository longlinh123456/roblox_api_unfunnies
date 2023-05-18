use anyhow::{ensure, Context, Result};
use clap::Parser;
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::warn;

use self::defaults::{
    DEFAULT_BATCH_WAIT, DEFAULT_CONFIG_PATH, DEFAULT_CONNECT_TIMEOUT, DEFAULT_DETAILED_WAIT,
    DEFAULT_FUNDS_THRESHOLD, DEFAULT_HTTP_PATH, DEFAULT_RETRY_LIMIT, DEFAULT_SOCKS5_PATH,
    DEFAULT_TIMEOUT,
};

mod defaults;

#[derive(Debug)]
pub struct Config {
    pub settings: Settings,
    pub proxies: String,
}

#[derive(Debug)]
pub struct Settings {
    pub retry_limit: usize,
    pub browser_id: String,
    pub funds_threshold: u64,
    pub cookie: String,
    pub user_agent: String,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub batch_wait: Duration,
    pub detailed_wait: Duration,
}

#[derive(Serialize, Deserialize, Debug, Parser)]
#[command(name = "roblox_group_finder")]
#[command(version, about, long_about = None)]
struct Args {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "retry")]
    retry_limit: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "tracker")]
    browser_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "cookie")]
    cookie: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "funds")]
    funds_threshold: Option<u64>,

    #[serde(rename = "socks5")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "socks5")]
    socks5_path: Option<PathBuf>,

    #[serde(rename = "http")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "http")]
    http_path: Option<PathBuf>,

    #[serde(skip_deserializing)]
    #[arg(long = "config", default_value_os_t = PathBuf::from(DEFAULT_CONFIG_PATH))]
    config_path: PathBuf,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "user_agent")]
    user_agent: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "timeout")]
    timeout: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "connect_timeout")]
    connect_timeout: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "batch_wait")]
    batch_wait: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long = "detailed_wait")]
    detailed_wait: Option<u64>,
}

fn parse_args() -> Result<Args> {
    let args = Args::parse();
    let figment = Figment::new();
    if let Ok(config) = fs::read_to_string(&args.config_path) {
        figment.join(Toml::string(&config))
    } else {
        warn!("Failed to read config at {}", &args.config_path.display());
        figment
    }
    .join(Serialized::defaults(Args {
        http_path: Some(DEFAULT_HTTP_PATH.into()),
        socks5_path: Some(DEFAULT_SOCKS5_PATH.into()),
        retry_limit: Some(DEFAULT_RETRY_LIMIT),
        browser_id: None,
        funds_threshold: Some(DEFAULT_FUNDS_THRESHOLD),
        config_path: PathBuf::new(),
        user_agent: None,
        cookie: None,
        timeout: Some(DEFAULT_TIMEOUT),
        connect_timeout: Some(DEFAULT_CONNECT_TIMEOUT),
        batch_wait: Some(DEFAULT_BATCH_WAIT),
        detailed_wait: Some(DEFAULT_DETAILED_WAIT),
    }))
    .extract::<Args>()
    .with_context(|| "Failed to merge CLI args and config files")
}

fn try_read_file(path: &Path, file_content: &str) -> String {
    fs::read_to_string(path).map_or_else(
        |_| {
            warn!("Failed to read {} at {}", file_content, path.display());
            String::new()
        },
        |contents| contents,
    )
}

pub fn get_config() -> Result<Config> {
    let args = parse_args()?;
    ensure!(args.browser_id.is_some(), "No browser ID provided");
    ensure!(args.cookie.is_some(), "No group claimer account provided");

    let http_proxies = try_read_file(&args.http_path.unwrap(), "http proxies")
        .lines()
        .map(|proxy| format!("http://{proxy}"))
        .join("\n");
    let socks5_proxies = try_read_file(&args.socks5_path.unwrap(), "socks5 proxies")
        .lines()
        .map(|proxy| format!("socks5://{proxy}"))
        .join("\n");

    let proxies = [http_proxies, socks5_proxies]
        .into_iter()
        .filter(|proxies| !proxies.is_empty())
        .join("\n");

    Ok(Config {
        settings: Settings {
            retry_limit: args.retry_limit.unwrap(),
            browser_id: args.browser_id.unwrap(),
            funds_threshold: args.funds_threshold.unwrap(),
            cookie: args.cookie.unwrap(),
            user_agent: args.user_agent.unwrap_or_default(),
            timeout: Duration::from_millis(args.timeout.unwrap()),
            connect_timeout: Duration::from_millis(args.connect_timeout.unwrap()),
            batch_wait: Duration::from_millis(args.batch_wait.unwrap()),
            detailed_wait: Duration::from_millis(args.detailed_wait.unwrap()),
        },
        proxies,
    })
}
