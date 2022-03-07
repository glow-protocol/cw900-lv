#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::staking::{
    apply_pending_slope_changes_to_state, apply_pending_slope_changes_to_state_and_save_updates,
    send_tokens, update_user_lock,
};
use crate::state::{
    Config, State, UserLockedBalance, CONFIG, MAX_SECONDS, MAX_WEEKS, SECONDS_PER_WEEK, STATE,
    USER_LOCKED_BALANCES,
};
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128,
};
use cw20::Cw20ReceiveMsg;

use glow_protocol::ve_token::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, StakerResponse,
    StateResponse,
};

pub fn is_contract(_addr: &Addr) -> bool {
    false
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        cw20_address: None,
        owner: deps.api.addr_validate(info.sender.as_str())?,
    };

    CONFIG.save(deps.storage, &config)?;

    // Save an initial default state
    let state = State::default();
    STATE.save(deps.storage, &state, env.block.time.seconds())?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Withdraw {} => execute_withdraw(deps, env, info),
        ExecuteMsg::IncreaseEndLockTime { end_lock_time } => {
            execute_increase_end_lock_time(deps, env, info, end_lock_time)
        }
        ExecuteMsg::Checkpoint {} => execute_global_checkpoint(deps, env, info),
        ExecuteMsg::RegisterContracts { cw20_address } => {
            execute_register_contracts(deps, cw20_address)
        }
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    // only asset contract can execute this message
    let config: Config = CONFIG.load(deps.storage)?;

    match config.cw20_address {
        Some(cw20_address) if cw20_address == deps.api.addr_validate(info.sender.as_str())? => {}
        _ => {
            return Err(ContractError::Unauthorized {});
        }
    }

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::CreateLock { end_lock_time }) => {
            let api = deps.api;
            execute_create_lock(
                deps,
                env,
                api.addr_validate(&cw20_msg.sender)?,
                cw20_msg.amount,
                end_lock_time,
            )
        }

        Ok(Cw20HookMsg::IncreaseLockAmount {}) => {
            let api = deps.api;
            execute_increase_lock_amount(
                deps,
                env,
                api.addr_validate(&cw20_msg.sender)?,
                cw20_msg.amount,
            )
        }
        _ => Err(ContractError::DataShouldBeGiven {}),
    }
}

pub fn execute_global_checkpoint(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    apply_pending_slope_changes_to_state_and_save_updates(
        deps.storage,
        &mut state,
        env.block.time.seconds(),
    )?;

    Ok(Response::new())
}

pub fn execute_create_lock(
    deps: DepsMut,
    env: Env,
    user: Addr,
    amount: Uint128,
    end_lock_time: u64,
) -> Result<Response, ContractError> {
    let end_lock_time = end_lock_time / SECONDS_PER_WEEK * SECONDS_PER_WEEK;

    let prev_user_locked_balance = USER_LOCKED_BALANCES
        .may_load(deps.storage, &user)?
        .unwrap_or_default();

    // Validate that the address is not a contract
    if is_contract(&user) {
        return Err(ContractError::ContractsCannotInteractWithLocks {});
    }

    // Validate that the old lock is finished
    if prev_user_locked_balance.exists() {
        return Err(ContractError::LockAlreadyExists {});
    }

    // Validate that the new lock is positive
    if amount == Uint128::zero() {
        return Err(ContractError::InsufficientLockAmount {});
    }

    // Validate that the new lock is in the future
    if end_lock_time <= env.block.time.seconds() {
        return Err(ContractError::EndLockTimeTooEarly {});
    }

    // Validate that the unlock week isn't too far in the future
    if end_lock_time > env.block.time.seconds() + MAX_SECONDS {
        return Err(ContractError::EndLockTimeTooLate {
            max_weeks: MAX_WEEKS,
            lock_duration_in_weeks: (end_lock_time - env.block.time.seconds()) / MAX_WEEKS,
        });
    }

    // Create the new user ve token point
    let new_user_locked_balance = UserLockedBalance {
        // Locked balance info
        deposited_amount: amount,
        end_lock_time,
        start_lock_time: env.block.time.seconds(),
        // History tracking info
        timestamp: env.block.time.seconds(),
    };

    // Propogate the changes
    update_user_lock(
        deps.storage,
        &user,
        prev_user_locked_balance,
        new_user_locked_balance,
    )?;

    Ok(Response::new().add_attributes(vec![
        ("action", "create_lock"),
        ("user", user.as_str()),
        ("amount", amount.to_string().as_str()),
    ]))
}

pub fn execute_increase_end_lock_time(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_end_lock_time: u64,
) -> Result<Response, ContractError> {
    let user = info.sender;
    let new_end_lock_time = new_end_lock_time / SECONDS_PER_WEEK * SECONDS_PER_WEEK;

    let prev_user_locked_balance = USER_LOCKED_BALANCES
        .may_load(deps.storage, &user)?
        .unwrap_or_default();

    // Validate that the address is not a contract
    if is_contract(&user) {
        return Err(ContractError::ContractsCannotInteractWithLocks {});
    }

    // Validate that the lock exists
    if prev_user_locked_balance.is_void_or_undefined() {
        return Err(ContractError::LockDoesNotExist {});
    }

    // Validate that the current lock isn't expired.
    if prev_user_locked_balance.expired_at_timestamp(env.block.time.seconds()) {
        return Err(ContractError::LockIsExpired {});
    }

    // Validate that the unlock week is further in the future than the current end lock time
    if prev_user_locked_balance.end_lock_time >= new_end_lock_time {
        return Err(ContractError::EndLockTimeTooEarly {});
    }

    // Validate that you aren't increasing the lock period too far
    if new_end_lock_time > env.block.time.seconds() + MAX_SECONDS {
        return Err(ContractError::EndLockTimeTooLate {
            max_weeks: MAX_WEEKS,
            lock_duration_in_weeks: (new_end_lock_time - env.block.time.seconds()) / MAX_WEEKS,
        });
    }

    // Create the new user ve token point
    let new_user_locked_balance = UserLockedBalance {
        // Locked balance info
        deposited_amount: prev_user_locked_balance.deposited_amount,
        end_lock_time: new_end_lock_time,
        start_lock_time: env.block.time.seconds(),
        // History tracking info
        timestamp: env.block.time.seconds(),
    };

    // Propogate the changes
    update_user_lock(
        deps.storage,
        &user,
        prev_user_locked_balance,
        new_user_locked_balance,
    )?;

    Ok(Response::new())
}

pub fn execute_increase_lock_amount(
    deps: DepsMut,
    env: Env,
    user: Addr,
    increase_amount: Uint128,
) -> Result<Response, ContractError> {
    // Validate that the address is not a contract
    if is_contract(&user) {
        return Err(ContractError::ContractsCannotInteractWithLocks {});
    }

    let prev_user_locked_balance = USER_LOCKED_BALANCES
        .may_load(deps.storage, &user)?
        .unwrap_or_default();

    // Validate that a lock exists
    if prev_user_locked_balance.is_void_or_undefined() {
        return Err(ContractError::LockDoesNotExist {});
    }

    // Validate that the current lock isn't expired
    if prev_user_locked_balance.expired_at_timestamp(env.block.time.seconds()) {
        return Err(ContractError::LockIsExpired {});
    }

    // Validate that the amount to increase by is positive
    if increase_amount == Uint128::zero() {
        return Err(ContractError::InsufficientLockIncreaseAmount {});
    }

    // Create the new user ve token point
    let new_user_locked_balance = UserLockedBalance {
        // Locked balance info
        deposited_amount: prev_user_locked_balance.deposited_amount + increase_amount,
        end_lock_time: prev_user_locked_balance.end_lock_time,
        start_lock_time: env.block.time.seconds(),
        // History tracking info
        timestamp: env.block.time.seconds(),
    };

    // Propogate the changes
    update_user_lock(
        deps.storage,
        &user,
        prev_user_locked_balance,
        new_user_locked_balance,
    )?;

    Ok(Response::new())
}

pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let user = info.sender;

    // Validate that the address is not a contract
    if is_contract(&user) {
        return Err(ContractError::ContractsCannotInteractWithLocks {});
    }

    // Get the user locked balance
    let prev_user_locked_balance = USER_LOCKED_BALANCES
        .may_load(deps.storage, &user)?
        .unwrap_or_default();

    // Validate that the lock isn't void
    if prev_user_locked_balance.is_void_or_undefined() {
        return Err(ContractError::LockDoesNotExist {});
    }

    let new_user_locked_balance: UserLockedBalance;
    let withdrawn_amount: Uint128;

    if prev_user_locked_balance.expired_at_timestamp(env.block.time.seconds()) {
        // If the lock is expired, then withdraw the full amount
        withdrawn_amount = prev_user_locked_balance.deposited_amount;
        // Set the new user locked balance to be zeroed out
        new_user_locked_balance =
            UserLockedBalance::void_lock_with_timestamp(env.block.time.seconds());
    } else {
        // The lock is not expired, so withdraw everything that is available to be withdrawn

        // Get the locked_amount at the current timestamp
        let locked_amount =
            prev_user_locked_balance.locked_amount_at_timestamp(env.block.time.seconds());

        // The amount available to be withdrawn
        let unlocked_amount = prev_user_locked_balance.deposited_amount - locked_amount;

        withdrawn_amount = unlocked_amount;

        if withdrawn_amount == prev_user_locked_balance.deposited_amount {
            // Set the new user locked balance to be zeroed out
            // if the withdrawn amount happens to be the entire deposit amount
            new_user_locked_balance =
                UserLockedBalance::void_lock_with_timestamp(env.block.time.seconds());
        } else {
            new_user_locked_balance = UserLockedBalance {
                // Locked balance info
                deposited_amount: locked_amount,
                end_lock_time: prev_user_locked_balance.end_lock_time,
                // Reset the start_lock_time
                start_lock_time: env.block.time.seconds(),
                // History tracking info
                timestamp: env.block.time.seconds(),
            }
        }
    }

    // Propogate the changes
    update_user_lock(
        deps.storage,
        &user,
        prev_user_locked_balance,
        new_user_locked_balance,
    )?;

    let config = CONFIG.load(deps.storage)?;

    let cw20_address = if let Some(cw20_address) = config.cw20_address {
        cw20_address
    } else {
        return Err(ContractError::ConfigContractsNotRegistered {});
    };

    send_tokens(&cw20_address, &user, withdrawn_amount, "withdraw")
}

pub fn execute_register_contracts(
    deps: DepsMut,
    cw20_address: String,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    if config.cw20_address != None {
        return Err(ContractError::Unauthorized {});
    }

    config.cw20_address = Some(deps.api.addr_validate(&cw20_address)?);
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
        QueryMsg::State { timestamp } => Ok(to_binary(&query_state(deps, env, timestamp)?)?),
        QueryMsg::Staker { address, timestamp } => {
            Ok(to_binary(&query_staker(deps, env, address, timestamp)?)?)
        }
    }
}

fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        cw20_address: config
            .cw20_address
            .map_or("None".to_string(), |a| a.to_string()),
    })
}

fn query_state(
    deps: Deps,
    env: Env,
    timestamp: Option<u64>,
) -> Result<StateResponse, ContractError> {
    let timestamp = timestamp.unwrap_or_else(|| env.block.time.seconds());
    let mut state: State = STATE
        .may_load_at_height(deps.storage, timestamp)?
        .unwrap_or_default();

    apply_pending_slope_changes_to_state(deps.storage, &mut state, env.block.time.seconds())?;

    Ok(StateResponse {
        total_deposited_amount: state.total_deposit,
        total_locked_amount: state
            .voting_power_coefficients
            .evaluate_locked_balance_at_timestamp(timestamp),
        total_balance: state
            .voting_power_coefficients
            .evaluate_voting_power_at_timestamp(timestamp),
    })
}

pub fn query_staker(
    deps: Deps,
    env: Env,
    address: String,
    timestamp: Option<u64>,
) -> Result<StakerResponse, ContractError> {
    let timestamp = timestamp.unwrap_or_else(|| env.block.time.seconds());
    let staker_addr = deps.api.addr_validate(address.as_str())?;
    let user_locked_balance = USER_LOCKED_BALANCES
        .may_load_at_height(deps.storage, &staker_addr, timestamp)?
        .unwrap_or_default();

    Ok(StakerResponse {
        deposited_amount: user_locked_balance.deposited_amount,
        locked_amount: user_locked_balance.locked_amount_at_timestamp(timestamp),
        balance: user_locked_balance.voting_power_at_timestamp(timestamp),
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
