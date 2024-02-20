use std::{collections::HashMap, process, sync::atomic::Ordering};

use fxhash::FxBuildHasher;
use kanal::{AsyncReceiver, Receiver, Sender};
use roblox_api::{
    apis::{
        economy::EconomyAuthenticatedApi,
        groups::{GroupsApi, GroupsAuthenticatedApi, Metadata},
    },
    AuthenticatedClient, BaseClient, Error, Id,
};
use tokio::{
    task,
    time::{self, Instant},
};
use tracing::{error, info, warn};

use crate::{
    constants::{CAPTCHA_MESSAGE, RATE_LIMITED_MESSAGE},
    status_display::{
        BATCH_CHECK_COUNTER, BATCH_PROXIES, GROUPS_CLAIMED, GROUPS_OWNED, ROBUX_CLAIMED,
    },
    SETTINGS,
};

#[derive(Debug)]
pub struct TrackedGroup {
    pub id: Id,
    pub processed_before: bool,
}
impl Default for TrackedGroup {
    fn default() -> Self {
        Self {
            id: Id::MIN,
            processed_before: false,
        }
    }
}

#[allow(unused_must_use)]
pub async fn detailed_check(
    client: impl BaseClient + Send,
    check_receiver: Receiver<Id>,
    priority_check_queue: (Sender<Id>, Receiver<Id>),
    claim_sender: Sender<Id>,
) {
    let mut retry_count: usize = 0;
    let settings = SETTINGS.get().unwrap();
    loop {
        let current_group = loop {
            if let Some(group) = priority_check_queue.1.try_recv().unwrap() {
                break group;
            } else if let Some(group) = check_receiver.try_recv().unwrap() {
                break group;
            }
            task::yield_now().await;
        };

        let request_start = Instant::now();
        let response = client.get_detailed_info(current_group).await;
        let request_end = Instant::now();
        match response {
            Ok(group_info) => {
                retry_count = 0;
                if !claim_sender.is_full()
                    && group_info.public_entry_allowed
                    && group_info.owner.is_none()
                    && !group_info.is_locked
                {
                    claim_sender.send(current_group);
                }
            }
            Err(error) => {
                priority_check_queue.0.send(current_group);
                if let Error::Api(error) = error {
                    if error.message == RATE_LIMITED_MESSAGE {
                        continue;
                    }
                }
                if retry_count >= settings.retry_limit {
                    break;
                }
                retry_count += 1;
            }
        }
        time::sleep(
            settings
                .detailed_wait
                .saturating_sub(request_end - request_start),
        )
        .await;
    }
}

#[allow(unused_must_use)]
pub async fn batch_check(
    client: impl BaseClient + Send,
    check_queue: (Sender<TrackedGroup>, Receiver<TrackedGroup>),
    priority_check_queue: (Sender<TrackedGroup>, Receiver<TrackedGroup>),
    detailed_check_sender: Sender<Id>,
) {
    let mut retry_count: usize = 0;
    let settings = SETTINGS.get().unwrap();
    let mut current_batch: HashMap<Id, bool, FxBuildHasher> =
        HashMap::with_capacity_and_hasher(100, FxBuildHasher::default());
    loop {
        current_batch.clear();
        while current_batch.len() < 100 {
            let group = priority_check_queue
                .1
                .try_recv()
                .unwrap()
                .or_else(|| check_queue.1.try_recv().unwrap());
            match group {
                Some(group) => {
                    current_batch.insert(group.id, group.processed_before);
                }
                None => {
                    if !current_batch.is_empty() {
                        break;
                    }
                }
            }
        }

        let request_start = Instant::now();
        let response = client.get_batch_info(current_batch.keys()).await;
        let request_end = Instant::now();
        match response {
            Ok(data) => {
                retry_count = 0;
                for group_info in &data {
                    if group_info.owner.is_none() {
                        if *current_batch.get(&group_info.id).unwrap() {
                            detailed_check_sender.send(group_info.id);
                        }
                    } else {
                        check_queue.0.send(TrackedGroup {
                            id: group_info.id,
                            processed_before: true,
                        });
                    }
                }
                #[allow(clippy::cast_possible_truncation)]
                BATCH_CHECK_COUNTER.fetch_add(data.len() as u32, Ordering::Relaxed);
            }
            Err(error) => {
                for (id, tracked) in &current_batch {
                    priority_check_queue.0.send(TrackedGroup {
                        id: *id,
                        processed_before: *tracked,
                    });
                }
                if let Error::Api(error) = error {
                    if error.message == RATE_LIMITED_MESSAGE {
                        continue;
                    }
                }
                if retry_count >= settings.retry_limit {
                    break;
                }
                retry_count += 1;
            }
        }
        time::sleep(
            settings
                .batch_wait
                .saturating_sub(request_end - request_start),
        )
        .await;
    }
    BATCH_PROXIES.fetch_sub(1, Ordering::Relaxed);
}

pub async fn claim(
    client: impl AuthenticatedClient + Send,
    claim_receiver: AsyncReceiver<Id>,
    metadata: Metadata,
    user_id: Id,
) {
    let settings = SETTINGS.get().unwrap();
    GROUPS_OWNED.store(metadata.current_group_count, Ordering::Relaxed);
    loop {
        let current_group = claim_receiver.recv().await.unwrap();
        info!("Claiming group {}", current_group);
        match client.join_group(current_group, None).await {
            Ok(_) => match client.claim_group(current_group).await {
                Ok(_) => match client.get_group_funds(current_group).await {
                    Ok(funds) => {
                        if funds < settings.funds_threshold {
                            if client
                                .remove_user_from_group(current_group, user_id)
                                .await
                                .is_ok()
                            {
                                info!(
                                    "Left group {} with insufficient funds ({} robux)",
                                    current_group, funds
                                );
                            } else {
                                warn!(
                                    "Failed to leave group {} with insufficient funds ({} robux)",
                                    current_group, funds
                                );
                            }
                        } else {
                            info!(
                                "Successfully claimed group {} ({} robux)",
                                current_group, funds
                            );
                            #[allow(clippy::cast_possible_truncation)]
                            ROBUX_CLAIMED.fetch_add(funds as u32, Ordering::Relaxed);
                            GROUPS_CLAIMED.fetch_add(1, Ordering::Relaxed);
                            let current_group_count =
                                GROUPS_OWNED.fetch_add(1, Ordering::Relaxed) + 1;
                            if current_group_count >= metadata.group_limit {
                                info!("Account is at the group limit, terminating");
                                process::exit(0);
                            }
                        }
                    }
                    Err(error) => warn!(
                        "Failed to get funds for group {}, error: {:?}",
                        current_group, error
                    ),
                },
                Err(error) => warn!(
                    "Failed to claim group {}, error: {:?}",
                    current_group, error
                ),
            },
            Err(error) => {
                warn!("Failed to join group {}, error: {:?}", current_group, error);
                if let Error::Api(error) = error {
                    if error.message == CAPTCHA_MESSAGE {
                        error!("Browser ID is invalid");
                        process::exit(0);
                    }
                }
            }
        }
    }
}
