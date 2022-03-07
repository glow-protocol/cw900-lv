use crate::contract::{DEFAULT_CLAIM_LIMIT, SECONDS_PER_WEEK};
use crate::querier::{
    query_address_voting_balance_at_timestamp, query_total_voting_balance_at_timestamp,
};
use crate::state::{Config, USER_LAST_CLAIMED_FEE_TIMESTAMP, WEEKLY_TOKEN_DISTRIBUTION};
use std::convert::TryInto;

use cosmwasm_std::{Addr, Deps, Env, Order, StdResult, Uint128};
use cw_storage_plus::Bound;

pub fn compute_claimable(
    deps: Deps,
    env: Env,
    config: &Config,
    user: &Addr,
    limit: Option<u32>,
    start_after: Option<u64>,
) -> StdResult<(u64, u64, Uint128)> {
    // Set the initlal last claimed fee timestamp
    // if the user has never claimed a fee before, it will be unwrapped to default
    // which is 0.
    let initial_last_claimed_fee_timestamp = start_after.unwrap_or(
        USER_LAST_CLAIMED_FEE_TIMESTAMP
            .may_load(deps.storage, user.clone())?
            .unwrap_or_default(),
    );

    // Copy the initial_last_claimed_fee_timestamp.
    // We don't want to mutate the initial_last_claimed_fee_timestamp
    // so that we can send it back unchanged in the response..
    let mut last_claimed_fee_timestamp = initial_last_claimed_fee_timestamp;

    // Increaes the start_time by SECONDS_PER_WEEK to get to the next week.
    // If the user has never collected a fee, this will be set to
    // SECONDS_PER_WEEK
    let start_time = last_claimed_fee_timestamp + SECONDS_PER_WEEK;

    // Set the end time to the current week rounded down
    // minus SECONDS_PER_WEEK. This means it gets set to the rounded down version of one week ago.

    // If env.block.time.seconds is divisible by SECONDS_PER_WEEK
    // (which means right at the cut off)
    // go to the previous week.
    let end_time =
        env.block.time.seconds() / SECONDS_PER_WEEK * SECONDS_PER_WEEK - SECONDS_PER_WEEK;

    // Set limit, or DEFAULT_CLAIM_LIMIT if undefined.
    let limit = limit.unwrap_or(DEFAULT_CLAIM_LIMIT) as usize;

    // Do a range query over WEEKLY_TOKEN_DISTRIBUTION
    // starting with start_time inclusive (the week after the previous collection fee time)
    // and ending with end time inclusive (the cutoff of the week before this one).
    // Take a limit of the range query, map the key to the timestamp, and collect.
    let token_distributions = WEEKLY_TOKEN_DISTRIBUTION
        .range(
            deps.storage,
            Some(Bound::Inclusive(start_time.to_be_bytes().into())),
            Some(Bound::Inclusive(end_time.to_be_bytes().into())),
            Order::Ascending,
        )
        .take(limit)
        .map(|item| {
            let (k, v) = item?;

            let timestamp = u64::from_be_bytes(k.try_into().unwrap());

            Ok((timestamp, v))
        })
        .collect::<StdResult<Vec<_>>>()?;

    // Initialize claim_amount as set to 0
    let mut claim_amount = Uint128::zero();

    for (timestamp, distributed_amount) in token_distributions {
        // For each pair of timestamp and distributed_amount in token_distributions,
        // - update last_claimed_fee_timestamp.
        // - get the total voting balance at the corresponding time.
        // - get the uer's voting balance at the corresponding time.
        // - increase claim_amount by distributed_amount * (user_voting_balance / total_voting_balance)

        // Update last_claimed_fee_timestamp
        last_claimed_fee_timestamp = timestamp;

        // Get the total voting balance at this point in time
        let total_voting_balance = query_total_voting_balance_at_timestamp(
            &deps.querier,
            &config.ve_token,
            Some(timestamp),
        )?;

        // Get the user's voting balance at this point in time
        let user_voting_balance = query_address_voting_balance_at_timestamp(
            &deps.querier,
            &config.ve_token,
            Some(timestamp),
            user,
        )?;

        // Increment claim_ammount accordingly.
        claim_amount +=
            distributed_amount.multiply_ratio(user_voting_balance, total_voting_balance);
    }

    // Return the initial_last_claimed_fee_timestamp,
    // the last_claimed_fee_timestamp
    // and the claimed_amount
    Ok((
        initial_last_claimed_fee_timestamp,
        last_claimed_fee_timestamp,
        claim_amount,
    ))
}
