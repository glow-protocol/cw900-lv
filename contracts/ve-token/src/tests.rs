use crate::{
    contract::{execute, instantiate, query},
    error::ContractError,
    state::{
        UserLockedBalance, MAX_SECONDS, SECONDS_PER_WEEK, STATE, USER_LOCKED_BALANCES,
        VOTING_POWER_CONSTANT_DIVISOR,
    },
};
use cosmwasm_std::{
    from_binary,
    testing::{mock_dependencies, mock_env, mock_info},
    to_binary, Addr, CosmosMsg, DepsMut, Env, SubMsg, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use glow_protocol::ve_token::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StakerResponse, StateResponse,
};

const TEST_CREATOR: &str = "creator";
const VOTING_TOKEN: &str = "voting_token";
const TEST_VOTER: &str = "voter1";
const TEST_VOTER_2: &str = "voter2";
// const TEST_VOTER_3: &str = "voter3";
const BLOCKS_PER_SECOND: f64 = 0.16;

fn mock_instantiate(deps: DepsMut, env: Env) {
    let msg = InstantiateMsg {};

    let info = mock_info(TEST_CREATOR, &[]);
    let _res =
        instantiate(deps, env, info, msg).expect("contract successfully executes instantiateMsg");
}

fn mock_register_contracts(deps: DepsMut, env: Env) {
    let info = mock_info(TEST_CREATOR, &[]);
    let msg = ExecuteMsg::RegisterContracts {
        cw20_address: VOTING_TOKEN.to_string(),
    };
    let _res =
        execute(deps, env, info, msg).expect("contract successfully executes RegisterContracts");
}

fn mock_env_time(time: u64) -> Env {
    let mut env = mock_env();
    env.block.height = 100;
    env.block.time = Timestamp::from_seconds(time);
    env
}

fn increase_env_time(env: &mut Env, increase_time: u64) {
    env.block.time = Timestamp::from_seconds(env.block.time.seconds() + increase_time);
    env.block.height += (increase_time as f64 * BLOCKS_PER_SECOND) as u64;
}

#[test]
pub fn one_depositor_query_staker() {
    let mut env = mock_env_time(SECONDS_PER_WEEK);

    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let info = mock_info(VOTING_TOKEN, &[]);

    // Now try to lock up

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 20 weeks in the future
    let end_lock_time = env.block.time.seconds() + SECONDS_PER_WEEK * 20;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Read the staker info
    let staker_info: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    // Default because there is a lag time of one block before the change goes into effect.
    // Think more about this because really its a lag time of timestamp going up, can that have any unintended consequences?
    assert_eq!(staker_info, StakerResponse::default());

    increase_env_time(&mut env, 1);

    let staker_info: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    // hard coded
    let expected_staker_info = StakerResponse {
        deposited_amount: Uint128::from(1000000000u128),
        locked_amount: Uint128::from(999999918u128),
        balance: Uint128::from(384615321u128),
    };

    assert_eq!(staker_info, expected_staker_info);

    increase_env_time(&mut env, SECONDS_PER_WEEK / 2 - 1);

    let staker_info: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    // hard coded
    let expected_staker_info = StakerResponse {
        deposited_amount: Uint128::from(1000000000u128),
        locked_amount: Uint128::from(975000000u128),
        balance: Uint128::from(365624999u128),
    };
    assert_eq!(staker_info, expected_staker_info);

    // Fast forward to half way to the unlock period
    increase_env_time(&mut env, SECONDS_PER_WEEK / 2);
    increase_env_time(&mut env, SECONDS_PER_WEEK * 9);

    let staker_info: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let expected_staker_info = StakerResponse {
        // Deposited amount stays the same
        deposited_amount: Uint128::from(deposit_amount),
        // Locked amount is half of deposited_amount
        locked_amount: Uint128::from(deposit_amount / 2),
        // balance is locked_amount * remaining time / constant multiplier
        balance: Uint128::from(
            deposit_amount as u64 / 2 * SECONDS_PER_WEEK * 10 / VOTING_POWER_CONSTANT_DIVISOR,
        ),
    };

    assert_eq!(expected_staker_info, staker_info);

    // Fast forward to the second before the unlock period
    increase_env_time(&mut env, SECONDS_PER_WEEK * 10 - 1);

    let staker_info: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    // hardcoded
    let expected_staker_info = StakerResponse {
        // Deposited amount stays the same
        deposited_amount: Uint128::from(deposit_amount),
        // Locked amount is half of deposited_amount
        locked_amount: Uint128::from(83u128),
        // balance is locked_amount * remaining time / constant multiplier
        balance: Uint128::from(0u128),
    };

    assert_eq!(expected_staker_info, staker_info);

    // Fast forward to the unlock time
    increase_env_time(&mut env, 1);

    let staker_info: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let expected_staker_info = StakerResponse {
        // Deposited amount stays the same
        deposited_amount: Uint128::from(deposit_amount),
        // Locked amount is half of deposited_amount
        locked_amount: Uint128::from(0u128),
        // balance is locked_amount * remaining time / constant multiplier
        balance: Uint128::from(0u128),
    };

    assert_eq!(expected_staker_info, staker_info);

    // Go 1 second past the unlock time
    increase_env_time(&mut env, 1);

    let staker_info: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let expected_staker_info = StakerResponse {
        // Deposited amount stays the same
        deposited_amount: Uint128::from(deposit_amount),
        // Locked amount is half of deposited_amount
        locked_amount: Uint128::from(0u128),
        // balance is locked_amount * remaining time / constant multiplier
        balance: Uint128::from(0u128),
    };

    assert_eq!(expected_staker_info, staker_info);

    // Go 52 weeks + 1 second past the unlock time
    increase_env_time(&mut env, MAX_SECONDS);

    let staker_info: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let expected_staker_info = StakerResponse {
        // Deposited amount stays the same
        deposited_amount: Uint128::from(deposit_amount),
        // Locked amount is half of deposited_amount
        locked_amount: Uint128::from(0u128),
        // balance is locked_amount * remaining time / constant multiplier
        balance: Uint128::from(0u128),
    };

    assert_eq!(expected_staker_info, staker_info);
}

#[test]
pub fn voting_power_consistency() {
    let mut env = mock_env_time(SECONDS_PER_WEEK);
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let info = mock_info(VOTING_TOKEN, &[]);

    let user = Addr::unchecked(TEST_VOTER.to_string());

    // Now try to lock up

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 20 weeks in the future
    let end_lock_time = env.block.time.seconds() + SECONDS_PER_WEEK * 20;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Read the user's locked balance
    let user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap();

    // Make sure that the voting power stays close to the expected voting power for many iterations
    for _ in 0..10000 {
        // Voting power at timestamp should equal calculated voting power
        // They might be off by a little bit due to rounding errors.

        // voting_power is calculated using the corresponding quadratic equation
        let voting_power = user_locked_balance.voting_power_at_timestamp(env.block.time.seconds());
        // expected_voting_power is calculated using the simplified equation
        let expected_voting_power =
            calculate_voting_power_at_timestamp(&user_locked_balance, env.block.time.seconds());

        assert!(
            voting_power > expected_voting_power - Uint128::from(10u128)
                && voting_power < expected_voting_power + Uint128::from(10u128)
        );

        // Increase env time by a weird amount
        increase_env_time(&mut env, 127);
    }
}

#[test]
pub fn expired_at_edge() {
    // Set the time to right before the next week starts
    let env = mock_env_time(SECONDS_PER_WEEK - 1);
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let info = mock_info(VOTING_TOKEN, &[]);

    let user = Addr::unchecked(TEST_VOTER.to_string());

    // Now try to lock up

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 20 weeks in the future
    let end_lock_time = env.block.time.seconds() + 1;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Read the user's locked balance
    let user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap();

    // Test that the lock is not expired one second before expiration
    assert!(!user_locked_balance.expired_at_timestamp(env.block.time.seconds()));
    // Test that the lock is expired when the timestamp equals the end_lock_time
    assert!(user_locked_balance.expired_at_timestamp(env.block.time.seconds() + 1))
}

#[test]
pub fn test_underflow_overflow() {
    // - check with small deposit amounts
    // - check when depositing at a non rounded time
    // Set the time to right before the next week starts
    let mut env = mock_env_time(52 * 100 * SECONDS_PER_WEEK);

    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let info = mock_info(VOTING_TOKEN, &[]);

    let user = Addr::unchecked(TEST_VOTER.to_string());

    // Now try to lock up

    // Try to overflow with large amount of GLOW
    let deposit_amount: u128 = 100_000_000 * u128::pow(10, 6);

    // Sent the end_lock_time for 20 weeks in the future
    let end_lock_time = env.block.time.seconds() + MAX_SECONDS;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Read the user's locked balance
    let user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap();

    println!("Locked balance: {:?}", user_locked_balance);

    increase_env_time(&mut env, 1);

    // Read the staker info
    let staker_info: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    println!("Staker info: {:?}", staker_info);
}

#[test]
pub fn test_create_lock_validation() {
    // Set the time to right before the next week starts
    let env = mock_env_time(SECONDS_PER_WEEK - 1);
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let info = mock_info(VOTING_TOKEN, &[]);

    // Try and fail to lock up a zero amount

    let deposit_amount: u128 = 0;

    // Sent the end_lock_time for 20 weeks in the future
    let end_lock_time = env.block.time.seconds() + 1;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

    match execute_res {
        Err(ContractError::InsufficientLockAmount {}) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    // Try and fail to lock with end_lock_time in the past

    let deposit_amount: u128 = 1;

    // Sent the end_lock_time for 20 weeks in the future
    let end_lock_time = env.block.time.seconds() - 1;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

    match execute_res {
        Err(ContractError::EndLockTimeTooEarly {}) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    // Try and fail to lock with end_lock_time in the past

    let deposit_amount: u128 = 1;

    // Sent the end_lock_time for 20 weeks in the future
    let end_lock_time = env.block.time.seconds();

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

    match execute_res {
        Err(ContractError::EndLockTimeTooEarly {}) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    // Try to lock up for longer than 52 weeks.

    let deposit_amount: u128 = 1;

    // Sent the end_lock_time for 20 weeks in the future
    let end_lock_time = env.block.time.seconds() + MAX_SECONDS + 1;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

    match execute_res {
        Err(ContractError::EndLockTimeTooLate { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    // Succeed to lock up for 52 weeks with amount of 1

    let deposit_amount: u128 = 1;

    // Sent the end_lock_time for 20 weeks in the future
    let end_lock_time = env.block.time.seconds() + MAX_SECONDS;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Try and fail to create a lock when one already exists

    let deposit_amount: u128 = 1;

    // Sent the end_lock_time for 20 weeks in the future
    let end_lock_time = env.block.time.seconds() + MAX_SECONDS;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let execute_res = execute(deps.as_mut(), env, info, msg);

    match execute_res {
        Err(ContractError::LockAlreadyExists {}) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
pub fn test_increase_end_lock_time_validation() {
    // Set the time to right before the next week starts
    let env = mock_env_time(SECONDS_PER_WEEK);
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let token_info = mock_info(VOTING_TOKEN, &[]);

    let _user = Addr::unchecked(TEST_VOTER.to_string());

    // Try to increase the end lock time of a lock which doesn't exist.

    let voter_info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::IncreaseEndLockTime {
        end_lock_time: SECONDS_PER_WEEK * 3,
    };
    let res = execute(deps.as_mut(), env.clone(), voter_info.clone(), msg);

    match res {
        Err(ContractError::LockDoesNotExist {}) => {}
        _ => panic!("DO NOT ENTER"),
    };

    // Create a lock

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 3 weeks in the future
    let end_lock_time = SECONDS_PER_WEEK * 3;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env, token_info, msg).unwrap();

    // Expire the lock

    let env = mock_env_time(SECONDS_PER_WEEK * 4);

    // Try to increase the end lock time of an expired lock

    let msg = ExecuteMsg::IncreaseEndLockTime {
        end_lock_time: SECONDS_PER_WEEK * 5,
    };
    let res = execute(deps.as_mut(), env, voter_info.clone(), msg);

    match res {
        Err(ContractError::LockIsExpired {}) => {}
        _ => panic!("DO NOT ENTER"),
    };

    // Unexpire the lock

    let env = mock_env_time(SECONDS_PER_WEEK);

    // Try to decrease the end lock time / keep it the same

    let msg = ExecuteMsg::IncreaseEndLockTime {
        end_lock_time: SECONDS_PER_WEEK * 3,
    };
    let res = execute(deps.as_mut(), env.clone(), voter_info.clone(), msg);

    match res {
        Err(ContractError::EndLockTimeTooEarly {}) => {}
        _ => panic!("DO NOT ENTER"),
    };

    // Try to increase the end lock time too much

    let msg = ExecuteMsg::IncreaseEndLockTime {
        end_lock_time: SECONDS_PER_WEEK * 60,
    };
    let res = execute(deps.as_mut(), env.clone(), voter_info.clone(), msg);

    match res {
        Err(ContractError::EndLockTimeTooLate { .. }) => {}
        _ => panic!("DO NOT ENTER"),
    };

    // Increase the end lock time successfully

    let msg = ExecuteMsg::IncreaseEndLockTime {
        end_lock_time: SECONDS_PER_WEEK * 30,
    };
    let _res = execute(deps.as_mut(), env, voter_info, msg).unwrap();
}

#[test]
pub fn test_increase_lock_amount_validation() {
    // Set the time to right before the next week starts
    let env = mock_env_time(SECONDS_PER_WEEK - 1);
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let token_info = mock_info(VOTING_TOKEN, &[]);

    let increase_amount = 10_000_000;

    // Try to increase the amount of a lock which doesn't exist.

    // let voter_info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(increase_amount as u128),
        msg: to_binary(&Cw20HookMsg::IncreaseLockAmount {}).unwrap(),
    });
    let res = execute(deps.as_mut(), env.clone(), token_info.clone(), msg);

    match res {
        Err(ContractError::LockDoesNotExist {}) => {}
        _ => panic!("DO NOT ENTER"),
    };

    // Create a lock

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 3 weeks in the future
    let end_lock_time = SECONDS_PER_WEEK * 3;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env, token_info.clone(), msg).unwrap();

    // Expire the lock

    let env = mock_env_time(SECONDS_PER_WEEK * 4);

    // Try to increase the amount of an expired lock

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(increase_amount as u128),
        msg: to_binary(&Cw20HookMsg::IncreaseLockAmount {}).unwrap(),
    });
    let res = execute(deps.as_mut(), env, token_info.clone(), msg);

    match res {
        Err(ContractError::LockIsExpired {}) => {}
        _ => panic!("DO NOT ENTER"),
    };

    // Unexpire the lock

    let env = mock_env_time(SECONDS_PER_WEEK);

    // Try and fail to increase the lock by 0

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(0u128),
        msg: to_binary(&Cw20HookMsg::IncreaseLockAmount {}).unwrap(),
    });
    let res = execute(deps.as_mut(), env.clone(), token_info.clone(), msg);

    match res {
        Err(ContractError::InsufficientLockIncreaseAmount {}) => {}
        _ => panic!("DO NOT ENTER"),
    };

    // Succeed to increase the lock amount

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(increase_amount as u128),
        msg: to_binary(&Cw20HookMsg::IncreaseLockAmount {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), env, token_info, msg).unwrap();
}

#[test]
pub fn test_create_lock_user_locked_balances_update() {
    // Set the time to right before the next week starts
    let env = mock_env_time(SECONDS_PER_WEEK);
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let token_info = mock_info(VOTING_TOKEN, &[]);

    let user = Addr::unchecked(TEST_VOTER.to_string());

    // Read the user's locked balance
    let user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap_or_default();

    // Create a lock

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 3 weeks in the future
    let end_lock_time = SECONDS_PER_WEEK * 3;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env, token_info, msg).unwrap();

    // Read the user's locked balance
    let new_user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap();

    assert_eq!(user_locked_balance, UserLockedBalance::default());
    assert_eq!(
        new_user_locked_balance,
        UserLockedBalance {
            deposited_amount: Uint128::from(deposit_amount),
            end_lock_time,
            start_lock_time: SECONDS_PER_WEEK,
            timestamp: SECONDS_PER_WEEK
        }
    );
}

#[test]
pub fn test_increase_amount_user_locked_balances_update() {
    // Set the time to right before the next week starts
    let mut env = mock_env_time(SECONDS_PER_WEEK);
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let token_info = mock_info(VOTING_TOKEN, &[]);

    let user = Addr::unchecked(TEST_VOTER.to_string());

    // Create a lock

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 3 weeks in the future
    let end_lock_time = SECONDS_PER_WEEK * 3;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), token_info.clone(), msg).unwrap();

    increase_env_time(&mut env, SECONDS_PER_WEEK);

    // Read the user's locked balance
    let user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap_or_default();

    // Increase the amount
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::IncreaseLockAmount {}).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), token_info, msg).unwrap();

    // Read the user's locked balance
    let new_user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap();

    assert_eq!(
        new_user_locked_balance,
        UserLockedBalance {
            deposited_amount: Uint128::from(deposit_amount * 2),
            end_lock_time: user_locked_balance.end_lock_time,
            start_lock_time: SECONDS_PER_WEEK * 2,
            timestamp: SECONDS_PER_WEEK * 2
        }
    );
}

#[test]
pub fn test_increase_end_lock_time_user_locked_balances_update() {
    // Set the time to right before the next week starts
    let mut env = mock_env_time(SECONDS_PER_WEEK);
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let token_info = mock_info(VOTING_TOKEN, &[]);

    let user = Addr::unchecked(TEST_VOTER.to_string());

    // Create a lock

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 3 weeks in the future
    let end_lock_time = SECONDS_PER_WEEK * 3;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), token_info, msg).unwrap();

    increase_env_time(&mut env, SECONDS_PER_WEEK);

    // Read the user's locked balance
    let user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap_or_default();

    let voter_info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::IncreaseEndLockTime {
        end_lock_time: SECONDS_PER_WEEK * 4,
    };
    let _res = execute(deps.as_mut(), env.clone(), voter_info, msg).unwrap();

    // Read the user's locked balance
    let new_user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap();

    assert_eq!(
        new_user_locked_balance,
        UserLockedBalance {
            deposited_amount: user_locked_balance.deposited_amount,
            end_lock_time: SECONDS_PER_WEEK * 4,
            start_lock_time: SECONDS_PER_WEEK * 2,
            timestamp: SECONDS_PER_WEEK * 2
        }
    );
}

#[test]
pub fn test_full_withdraw_user_locked_balances_update() {
    // Set the time to right before the next week starts
    let mut env = mock_env_time(SECONDS_PER_WEEK);
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let token_info = mock_info(VOTING_TOKEN, &[]);

    let user = Addr::unchecked(TEST_VOTER.to_string());

    // Create a lock

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 3 weeks in the future
    let end_lock_time = SECONDS_PER_WEEK * 3;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), token_info, msg).unwrap();

    increase_env_time(&mut env, 4 * SECONDS_PER_WEEK);

    // Read the user's locked balance
    let _user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap();

    let voter_info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::Withdraw {};
    let res = execute(deps.as_mut(), env.clone(), voter_info, msg).unwrap();

    // Res should include a message to return all of the user's deposited glow

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: user.to_string(),
                amount: Uint128::from(deposit_amount),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // Read the user's locked balance
    let new_user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap();

    assert_eq!(
        new_user_locked_balance,
        UserLockedBalance::void_lock_with_timestamp(env.block.time.seconds())
    );
}

#[test]
pub fn test_partial_withdraw_user_locked_balances_update() {
    // Set the time to right before the next week starts
    let mut env = mock_env_time(SECONDS_PER_WEEK);
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let token_info = mock_info(VOTING_TOKEN, &[]);

    let user = Addr::unchecked(TEST_VOTER.to_string());

    // Create a lock

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 3 weeks in the future
    let end_lock_time = SECONDS_PER_WEEK * 3;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), token_info, msg).unwrap();

    increase_env_time(&mut env, SECONDS_PER_WEEK);

    // Read the user's locked balance
    let user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap();

    let voter_info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::Withdraw {};
    let res = execute(deps.as_mut(), env.clone(), voter_info, msg).unwrap();

    // Res should include a message to return all of the user's deposited glow

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: user.to_string(),
                amount: Uint128::from(deposit_amount / 2),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // Read the user's locked balance
    let new_user_locked_balance = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user)
        .unwrap();

    assert_eq!(
        new_user_locked_balance,
        UserLockedBalance {
            deposited_amount: Uint128::from(deposit_amount / 2),
            end_lock_time: user_locked_balance.end_lock_time,
            start_lock_time: env.block.time.seconds(),
            timestamp: env.block.time.seconds()
        }
    );
}

#[test]
pub fn two_depositors_query_total_balance() {
    // Set the time to right before the next week starts
    let mut env = mock_env_time(SECONDS_PER_WEEK);
    // Create a lock

    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut(), env.clone());
    mock_register_contracts(deps.as_mut(), env.clone());

    let token_info = mock_info(VOTING_TOKEN, &[]);

    let user1 = Addr::unchecked(TEST_VOTER.to_string());
    let user2 = Addr::unchecked(TEST_VOTER_2.to_string());

    // Create a lock

    // Stake 1000 GLOW
    let deposit_amount: u128 = 1000 * u128::pow(10, 6);

    // Sent the end_lock_time for 3 weeks in the future
    let end_lock_time = SECONDS_PER_WEEK * 5;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), token_info.clone(), msg).unwrap();

    // Create a second lock

    // Stake 1000 GLOW
    let deposit_amount: u128 = 2000 * u128::pow(10, 6);

    // Sent the end_lock_time for 3 weeks in the future
    let end_lock_time = SECONDS_PER_WEEK + MAX_SECONDS;

    // Create the lock
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(deposit_amount as u128),
        msg: to_binary(&Cw20HookMsg::CreateLock { end_lock_time }).unwrap(),
    });
    let _execute_res = execute(deps.as_mut(), env.clone(), token_info, msg).unwrap();

    // Verify that the total balance equals the sum of the individual balances
    // at multiple different times

    // Read the user's locked balance
    let user_locked_balance_1 = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user1)
        .unwrap();

    // Read the user's locked balance
    let user_locked_balance_2 = USER_LOCKED_BALANCES
        .load(deps.as_mut().storage, &user2)
        .unwrap();

    let user1_power =
        calculate_voting_power_at_timestamp(&user_locked_balance_1, env.block.time.seconds());

    let user2_power =
        calculate_voting_power_at_timestamp(&user_locked_balance_2, env.block.time.seconds());

    let state = STATE.load(deps.as_ref().storage).unwrap();

    assert_eq!(
        user1_power + user2_power,
        state
            .voting_power_coefficients
            .evaluate_voting_power_at_timestamp(env.block.time.seconds())
    );

    // Increase the env time

    increase_env_time(&mut env, SECONDS_PER_WEEK * 3 / 2);

    // Get staker and state info

    let staker_info_1: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let staker_info_2: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER_2.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let state_info: StateResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::State { timestamp: None },
        )
        .unwrap(),
    )
    .unwrap();

    assert!(
        staker_info_1.balance + staker_info_2.balance - Uint128::from(10u128)
            < state_info.total_balance
            && staker_info_1.balance + staker_info_2.balance + Uint128::from(10u128)
                > state_info.total_balance
    );

    assert!(
        staker_info_1.locked_amount + staker_info_2.locked_amount - Uint128::from(10u128)
            < state_info.total_locked_amount
            && staker_info_1.locked_amount + staker_info_2.locked_amount + Uint128::from(10u128)
                > state_info.total_locked_amount
    );

    // Expire the first lock and check again

    increase_env_time(&mut env, SECONDS_PER_WEEK * 3 / 2);
    increase_env_time(&mut env, SECONDS_PER_WEEK * 6);

    // Get staker info and state

    let staker_info_1: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let staker_info_2: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER_2.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let state_info: StateResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::State { timestamp: None },
        )
        .unwrap(),
    )
    .unwrap();

    assert!(
        staker_info_1.balance + staker_info_2.balance - Uint128::from(10u128)
            < state_info.total_balance
            && staker_info_1.balance + staker_info_2.balance + Uint128::from(10u128)
                > state_info.total_balance
    );

    assert!(
        staker_info_1.locked_amount + staker_info_2.locked_amount - Uint128::from(10u128)
            < state_info.total_locked_amount
            && staker_info_1.locked_amount + staker_info_2.locked_amount + Uint128::from(10u128)
                > state_info.total_locked_amount
    );

    // Increase the amount of the second lock

    let token_info = mock_info(VOTING_TOKEN, &[]);

    let increase_amount = 10_000_000;

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(increase_amount as u128),
        msg: to_binary(&Cw20HookMsg::IncreaseLockAmount {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), env.clone(), token_info, msg);

    // Check that everything is still good

    let staker_info_1: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let staker_info_2: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER_2.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let state_info: StateResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::State { timestamp: None },
        )
        .unwrap(),
    )
    .unwrap();

    assert!(
        staker_info_1.balance + staker_info_2.balance - Uint128::from(10u128)
            < state_info.total_balance
            && staker_info_1.balance + staker_info_2.balance + Uint128::from(10u128)
                > state_info.total_balance
    );

    assert!(
        staker_info_1.locked_amount + staker_info_2.locked_amount - Uint128::from(10u128)
            < state_info.total_locked_amount
            && staker_info_1.locked_amount + staker_info_2.locked_amount + Uint128::from(10u128)
                > state_info.total_locked_amount
    );

    // Increase env time

    increase_env_time(&mut env, SECONDS_PER_WEEK * 3 / 2);

    // Check that everything is still good

    let staker_info_1: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let staker_info_2: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER_2.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let state_info: StateResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::State { timestamp: None },
        )
        .unwrap(),
    )
    .unwrap();

    assert!(
        staker_info_1.balance + staker_info_2.balance - Uint128::from(10u128)
            < state_info.total_balance
            && staker_info_1.balance + staker_info_2.balance + Uint128::from(10u128)
                > state_info.total_balance
    );

    assert!(
        staker_info_1.locked_amount + staker_info_2.locked_amount - Uint128::from(10u128)
            < state_info.total_locked_amount
            && staker_info_1.locked_amount + staker_info_2.locked_amount + Uint128::from(10u128)
                > state_info.total_locked_amount
    );

    // Expire the second lock and check again

    increase_env_time(&mut env, SECONDS_PER_WEEK * 50);

    let staker_info_1: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let staker_info_2: StakerResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Staker {
                address: TEST_VOTER_2.to_string(),
                timestamp: None,
            },
        )
        .unwrap(),
    )
    .unwrap();

    let state_info: StateResponse = from_binary(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::State { timestamp: None },
        )
        .unwrap(),
    )
    .unwrap();

    assert!(staker_info_1.balance + staker_info_2.balance == state_info.total_balance);
    assert!(
        staker_info_1.locked_amount + staker_info_2.locked_amount == state_info.total_locked_amount
    );
}

pub fn calculate_voting_power_at_timestamp(
    locked_balance: &UserLockedBalance,
    timestamp: u64,
) -> Uint128 {
    locked_balance.locked_amount_at_timestamp(timestamp)
        * Uint128::from(locked_balance.end_lock_time - timestamp)
        / Uint128::from(VOTING_POWER_CONSTANT_DIVISOR)
}
