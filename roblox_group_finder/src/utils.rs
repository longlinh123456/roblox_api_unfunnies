use std::num::NonZeroUsize;

use async_trait::async_trait;
use roblox_api::apis::{self, groups::GroupsApi, Id, RequestResult};

use crate::constants;

#[must_use]
fn get_partitioning_ids(
    low_id: Id,
    high_id: Id,
    number_of_partitioning_ids: NonZeroUsize,
) -> Vec<Id> {
    assert!(
        low_id < high_id,
        "Tried to get partitioning ids between a low id not lower than high id"
    );

    let (number_of_partitioning_ids, high_id, low_id) = (
        number_of_partitioning_ids.get() as u64,
        high_id.get(),
        low_id.get(),
    );

    let search_space = high_id - low_id - 1;
    if search_space <= number_of_partitioning_ids {
        return (low_id + 1..high_id)
            .map(|id| Id::new(id).unwrap())
            .collect();
    }

    #[allow(clippy::cast_possible_truncation)]
    let mut partitioning_ids = Vec::with_capacity(number_of_partitioning_ids as usize);

    let space_to_partition = search_space - number_of_partitioning_ids;
    let number_of_partitions = number_of_partitioning_ids + 1;
    let partition_size = space_to_partition / number_of_partitions;
    let mut leftover_space = space_to_partition % number_of_partitions;
    let mut last_partitioning_id = low_id;
    for _ in 1..=number_of_partitioning_ids {
        let mut partitioning_id = last_partitioning_id + partition_size + 1;
        if leftover_space > 0 {
            partitioning_id += 1;
            leftover_space -= 1;
        }
        partitioning_ids.push(Id::new(partitioning_id).unwrap());
        last_partitioning_id = partitioning_id;
    }
    partitioning_ids
}

#[async_trait]
pub trait GroupsApiExt: apis::groups::GroupsApi {
    async fn get_latest_group_id(&self) -> RequestResult<Id> {
        let (mut low_id, mut high_id) = (Id::MIN, Id::MAX);
        while high_id - low_id > 1 {
            let ids_to_check = get_partitioning_ids(
                low_id,
                high_id,
                NonZeroUsize::new(constants::MAX_IDS_IN_BATCH_REQUEST).unwrap(),
            );
            match self.get_batch_info(ids_to_check.iter().copied()).await {
                Ok(results) => {
                    let max_present_id = results.last().map(|group_info| group_info.id);
                    high_id = max_present_id.map_or_else(
                        || ids_to_check[0],
                        |max_present_id| {
                            low_id = max_present_id;
                            *ids_to_check
                                .get(
                                    ids_to_check
                                        .iter()
                                        .position(|&id| id == max_present_id)
                                        .unwrap()
                                        + 1,
                                )
                                .unwrap_or(&high_id)
                        },
                    );
                }
                Err(error) => {
                    return Err(error);
                }
            }
        }
        Ok(low_id)
    }
}
impl<T: GroupsApi> GroupsApiExt for T {}
