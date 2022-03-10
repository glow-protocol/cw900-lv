use crate::error::ContractError;
use crate::state::{
    State, UserLockedBalance, COEFFICIENT_CHANGES, SECONDS_PER_WEEK, STATE, USER_LOCKED_BALANCES,
};

use cosmwasm_std::{to_binary, Addr, CosmosMsg, Response, StdResult, Storage, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;
use cw_storage_plus::U64Key;

pub fn update_user_lock(
    storage: &mut dyn Storage,
    user: &Addr,
    prev_user_locked_balance: UserLockedBalance,
    new_user_locked_balance: UserLockedBalance,
) -> StdResult<()> {
    // When creating a new lock
    // prev_user_locked_balance doesn't exist.
    // new_user_locked_balance is has positive amount and unlocks in the future

    // When increasing the end_lock_time of an existing lock
    // prev_user_locked_balance has positive amount and unlocks in the future
    // new_user_locked_balance has positive amount and unlocks in the future

    // When increasing the amount of an existing lock
    // prev_user_locked_balance has positive amount and unlocks in the future
    // new_user_locked_balance has positive amount and unlocks in the future

    // When decreasing the amount of an existing lock
    // prev_user_locked_balance has positive amount and unlocks in the future
    // new_user_locked_balance has positive amount and unlocks in the future

    // When deleting a lock
    // prev_user_locked_balance has positive amount and unlocks in the future
    // new_user_locked_balance doesn't exist.

    // Update the last global point
    let mut state = STATE.load(storage)?;

    // Mutate state to apply pending slope changes to state between state.timestamp and block_timestamp,
    // and save each updated state to storage.
    apply_pending_slope_changes_to_state_and_save_updates(
        storage,
        &mut state,
        new_user_locked_balance.timestamp,
    )?;

    // Update the state
    // to reflect the update to the lock
    update_state_for_lock_update(
        &mut state,
        &prev_user_locked_balance,
        &new_user_locked_balance,
    );

    // Update slope changes to schedule a reversal of the changes to made to state in update_state_for_lock_update
    update_slope_changes_for_lock_update(
        storage,
        &prev_user_locked_balance,
        &new_user_locked_balance,
    )?;

    // Save the updated state
    STATE.save(storage, &state, state.timestamp)?;

    // Save the new user ve token point
    USER_LOCKED_BALANCES.save(
        storage,
        user,
        &new_user_locked_balance,
        new_user_locked_balance.timestamp,
    )?;

    Ok(())
}

/// Apply pending slope changes to state between state.timestamp and block_timestamp
pub fn apply_pending_slope_changes_to_state(
    storage: &dyn Storage,
    state: &mut State,
    timestamp: u64,
) -> StdResult<()> {
    internal_apply_pending_slope_changes_to_state(
        IMStorage::ImmutableStorage(storage),
        state,
        timestamp,
    )
}

/// Apply pending slope changes to state between state.timestamp and block_timestamp,
/// and save each updated state to storage.
pub fn apply_pending_slope_changes_to_state_and_save_updates(
    storage: &mut dyn Storage,
    state: &mut State,
    timestamp: u64,
) -> StdResult<()> {
    internal_apply_pending_slope_changes_to_state(
        IMStorage::MutableStorage(storage),
        state,
        timestamp,
    )
}

/// Enum for allowing user to pass immutable or mutable storage to a function
/// and changing the logic of the function accordingly
enum IMStorage<'a> {
    ImmutableStorage(&'a dyn Storage),
    MutableStorage(&'a mut dyn Storage),
}

/// Apply pending slope changes to state between state.timestamp and block_timestamp.
/// If imstorage is of type IMStorage::MutableStorage, then save each updated state to storage.
fn internal_apply_pending_slope_changes_to_state(
    mut imstorage: IMStorage,
    state: &mut State,
    timestamp: u64,
) -> StdResult<()> {
    // Get the week that comes before the state's timestamp
    let mut week_iterator_timestamp = state.timestamp / SECONDS_PER_WEEK * SECONDS_PER_WEEK;

    // Go to the next week because we already processed
    // all weeks at or before the state's timestamp
    week_iterator_timestamp += SECONDS_PER_WEEK;

    // Loop to update state.
    for _ in 0..255 {
        if week_iterator_timestamp > timestamp {
            // We are past the current block timestamp, so break out of the loop
            break;
        }

        // Get the coefficient change corresponding to the week_iterator_timestamp
        let coefficient_changes = COEFFICIENT_CHANGES
            .may_load(
                match &imstorage {
                    IMStorage::ImmutableStorage(x) => *x,
                    IMStorage::MutableStorage(x) => *x,
                },
                U64Key::from(week_iterator_timestamp),
            )?
            .unwrap_or_default();

        // Subtract the coefficient changes from the total_balance_coefficients
        state.voting_power_coefficients -= coefficient_changes;

        if let IMStorage::MutableStorage(storage) = &mut imstorage {
            // Set the timestamp to that corresponding to the iterator
            state.timestamp = week_iterator_timestamp;

            // Save the state to storage at the corresponding timestamp
            STATE.save(*storage, state, state.timestamp)?;
        }

        // Increment week_interator
        week_iterator_timestamp += SECONDS_PER_WEEK;
    }

    Ok(())
}

/// Update state to incorporate lock update changes
pub fn update_state_for_lock_update(
    state: &mut State,
    prev_user_locked_balance: &UserLockedBalance,
    new_user_locked_balance: &UserLockedBalance,
) {
    // Okay now we can update the state

    // Only remove the prev coefficients if the lock isn't expired
    // Otherwise the prev coefficients were already removed as part of coefficient changes
    if !prev_user_locked_balance.expired_at_timestamp(new_user_locked_balance.timestamp) {
        // Remove prev token slope and bias
        state.voting_power_coefficients -= prev_user_locked_balance.voting_power_coefficients();
    }

    // Remove prev point deposited amount
    // This is always removed even if prev_user_locked_balance is expired
    // Because deposited_amount removal isn't scheduled as part of slope changes
    // It only takes place when a user withdraws.
    state.total_deposit -= prev_user_locked_balance.deposited_amount;

    // Add new token slope and bias
    state.voting_power_coefficients += new_user_locked_balance.voting_power_coefficients();

    // Add new point deposited amount
    state.total_deposit += new_user_locked_balance.deposited_amount;

    // Update the timestamp of the state to match the new locked balance
    state.timestamp = new_user_locked_balance.timestamp;
}

/// Update slope changes to schedule a reversal of the changes to made to state in
/// `update_state_for_lock_update`
pub fn update_slope_changes_for_lock_update(
    storage: &mut dyn Storage,
    prev_user_locked_balance: &UserLockedBalance,
    new_user_locked_balance: &UserLockedBalance,
) -> StdResult<()> {
    // Get old slope
    let mut old_coefficient_changes = COEFFICIENT_CHANGES
        .may_load(
            storage,
            U64Key::from(prev_user_locked_balance.end_lock_time),
        )?
        .unwrap_or_default();

    // Remove prev token point slope
    old_coefficient_changes -= prev_user_locked_balance.voting_power_coefficients();

    if new_user_locked_balance.end_lock_time == prev_user_locked_balance.end_lock_time {
        // If new token point ends at the same location, update old coefficient changes accordingly

        // Add new coefficient changes

        old_coefficient_changes += new_user_locked_balance.voting_power_coefficients();
    } else {
        // If new token points ends at a new location,
        // read the corresponding slope, update it, and save it
        let mut new_coefficient_changes = COEFFICIENT_CHANGES
            .may_load(storage, U64Key::from(new_user_locked_balance.end_lock_time))?
            .unwrap_or_default();

        // Add new coefficient changes
        new_coefficient_changes += new_user_locked_balance.voting_power_coefficients();

        COEFFICIENT_CHANGES.save(
            storage,
            U64Key::from(new_user_locked_balance.end_lock_time),
            &new_coefficient_changes,
        )?;
    }

    // Save old_dslope
    COEFFICIENT_CHANGES.save(
        storage,
        U64Key::from(prev_user_locked_balance.end_lock_time),
        &old_coefficient_changes,
    )?;

    Ok(())
}

/// Send `amount` tokens of type `asset_token` to `recipient`
pub fn send_tokens(
    asset_token: &Addr,
    recipient: &Addr,
    amount: Uint128,
    action: &str,
) -> Result<Response, ContractError> {
    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: recipient.to_string(),
                amount,
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            ("action", action),
            ("recipient", recipient.to_string().as_str()),
            ("amount", amount.to_string().as_str()),
        ]))
}
