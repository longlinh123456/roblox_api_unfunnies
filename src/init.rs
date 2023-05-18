use std::sync::atomic::Ordering;

use indicatif::ProgressBar;
use kanal::{Receiver, Sender};
use roblox_api::{
    clients::{Client, ClientBuilder, Proxy},
    Id,
};
use tokio::task;
use tracing::{info, warn};

use crate::{
    status_display::{self, BATCH_PROXIES},
    threads::{self, TrackedGroup},
    SETTINGS,
};

#[allow(clippy::cast_possible_truncation)]
pub fn init_check_threads(
    latest_group_id: Id,
    bar: &ProgressBar,
    proxies: &str,
    group_limit: u16,
) -> Receiver<Id> {
    let settings = SETTINGS.get().unwrap();
    let batch_check_queue: (Sender<TrackedGroup>, Receiver<TrackedGroup>) =
        kanal::bounded(latest_group_id.get() as usize);
    let batch_priority_check_queue: (Sender<TrackedGroup>, Receiver<TrackedGroup>) =
        kanal::bounded(latest_group_id.get() as usize);
    let detailed_check_queue: (Sender<Id>, Receiver<Id>) = kanal::unbounded();
    let detailed_priority_check_queue: (Sender<Id>, Receiver<Id>) = kanal::unbounded();
    let claim_queue: (Sender<Id>, Receiver<Id>) = kanal::unbounded();

    {
        let batch_senders = (
            batch_check_queue.0.clone(),
            batch_priority_check_queue.0.clone(),
        );
        info!("Starting status display");

        task::spawn(status_display::status_thread(
            bar.clone(),
            group_limit,
            batch_senders,
        ));
    }

    info!("Initializing check queue");
    for id in 1..=latest_group_id.get() {
        batch_check_queue
            .0
            .send(TrackedGroup {
                id: Id::new(id).unwrap(),
                processed_before: false,
            })
            .unwrap();
    }

    info!("Starting check tasks");
    let proxies = proxies.lines().collect::<Vec<&str>>();
    BATCH_PROXIES.store(proxies.len() as u64, Ordering::Relaxed);
    for proxy in proxies {
        if let Ok(proxy) = Proxy::all(proxy) {
            let client = Client::new(
                ClientBuilder::new()
                    .proxy(proxy)
                    .connect_timeout(settings.connect_timeout)
                    .timeout(settings.timeout)
                    .http2_prior_knowledge(),
            );
            task::spawn(threads::batch_check(
                client.clone(),
                batch_check_queue.clone(),
                batch_priority_check_queue.clone(),
                detailed_check_queue.0.clone(),
            ));
            task::spawn(threads::detailed_check(
                client,
                detailed_check_queue.1.clone(),
                (
                    detailed_priority_check_queue.0.clone(),
                    detailed_priority_check_queue.1.clone(),
                ),
                claim_queue.0.clone(),
            ));
        } else {
            warn!("Failed to create task with proxy URL {}", proxy);
        }
    }
    info!("Finished starting check tasks");
    claim_queue.1
}
