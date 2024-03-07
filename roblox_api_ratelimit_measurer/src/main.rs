#![deny(
    clippy::suspicious,
    clippy::complexity,
    clippy::perf,
    clippy::style,
    clippy::pedantic,
    clippy::correctness,
    clippy::nursery
)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::unreadable_literal
)]

use std::time::{Duration, Instant};

use roblox_api::{
    apis::{groups::GroupsApi, Error::Api, Id},
    clients::{Client, ClientBuilder},
};

const RATE_LIMITED_MESSAGE: &str = "Too many requests";
const TRIALS: u64 = 20;

async fn batch_measure(client: Client) {
    let mut times: u64 = 0;
    let mut added_time = Duration::from_millis(0);
    for _ in 1..=TRIALS {
        let first_timestamp = Instant::now();
        loop {
            if let Err(Api(info)) = client
                .get_batch_info((1..=100).map(|x| Id::new(x).unwrap()))
                .await
            {
                if info.message == RATE_LIMITED_MESSAGE {
                    break;
                }
            } else {
                times += 1;
            }
        }
        let second_timestamp = Instant::now();
        println!(
            "Made {} requests in {} ms",
            times,
            (second_timestamp - first_timestamp + added_time).as_millis()
        );
        times = 1;
        let first_timestamp = Instant::now();
        let mut last_timestamp = Instant::now();
        loop {
            if client
                .get_batch_info((1..=100).map(|x| Id::new(x).unwrap()))
                .await
                .is_ok()
            {
                println!(
                    "Rate limited for {} ms",
                    (last_timestamp - first_timestamp).as_millis()
                );
                added_time = last_timestamp.elapsed();
                break;
            }
            last_timestamp = Instant::now();
        }
    }
}

async fn detailed_measure(client: Client) {
    let mut times: u64 = 0;
    let mut added_time = Duration::from_millis(0);
    for _ in 1..=TRIALS {
        let first_timestamp = Instant::now();
        loop {
            if let Err(Api(info)) = client.get_detailed_info(Id::new(1).unwrap()).await {
                if info.message == RATE_LIMITED_MESSAGE {
                    break;
                }
            } else {
                times += 1;
            }
        }
        let second_timestamp = Instant::now();
        println!(
            "Made {} requests in {} ms",
            times,
            (second_timestamp - first_timestamp + added_time).as_millis()
        );
        times = 1;
        let first_timestamp = Instant::now();
        let mut last_timestamp = Instant::now();
        loop {
            if client.get_detailed_info(Id::new(1).unwrap()).await.is_ok() {
                println!(
                    "Rate limited for {} ms",
                    (last_timestamp - first_timestamp).as_millis()
                );
                added_time = last_timestamp.elapsed();
                break;
            }
            last_timestamp = Instant::now();
        }
    }
}

#[tokio::main]
async fn main() {
    let client = Client::new(ClientBuilder::new().no_proxy().http2_prior_knowledge());
    println!("Starting measure");
    println!("Measuring batch ratelimit");
    batch_measure(client.clone()).await;
    println!("Measuring detailed ratelimit");
    detailed_measure(client).await;
}
