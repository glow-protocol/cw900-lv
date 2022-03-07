use crate::contract::{execute, instantiate, query, SECONDS_PER_WEEK};
use crate::error::ContractError;
use crate::mock_querier::mock_dependencies;
use crate::state::{Config, State, CONFIG, STATE};

use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    coins, from_binary, to_binary, Addr, Api, CosmosMsg, DepsMut, Env, SubMsg, Timestamp, Uint128,
    WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use glow_protocol::fee_distributor::{ExecuteMsg, InstantiateMsg, QueryMsg, StakerResponse};

const VOTING_TOKEN: &str = "voting_token";
const VE_TOKEN: &str = "ve_token";
const TERRASWAP_FACTORY: &str = "terraswap_factory";
const TEST_CREATOR: &str = "creator";
const TEST_VOTER: &str = "voter1";
// const TEST_VOTER_2: &str = "voter2";
const BLOCKS_PER_SECOND: f64 = 0.16;

fn increase_env_time(env: &mut Env, increase_time: u64) {
    env.block.time = Timestamp::from_seconds(env.block.time.seconds() + increase_time);
    env.block.height += (increase_time as f64 * BLOCKS_PER_SECOND) as u64;
}

fn mock_instantiate(deps: DepsMut) {
    let msg = InstantiateMsg {};

    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps, mock_env(), info, msg)
        .expect("contract successfully executes instantiateMsg");
}

fn mock_register_contracts(deps: DepsMut) {
    let info = mock_info(TEST_CREATOR, &[]);
    let msg = ExecuteMsg::RegisterContracts {
        glow_token: VOTING_TOKEN.to_string(),
        ve_token: VE_TOKEN.to_string(),
        terraswap_factory: TERRASWAP_FACTORY.to_string(),
    };
    let _res = execute(deps, mock_env(), info, msg)
        .expect("contract successfully executes RegisterContracts");
}

fn mock_env_height(height: u64, time: u64) -> Env {
    let mut env = mock_env();
    env.block.height = height;
    env.block.time = Timestamp::from_seconds(time);
    env
}

fn instantiate_msg() -> InstantiateMsg {
    InstantiateMsg {}
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = instantiate_msg();
    let info = mock_info(TEST_CREATOR, &coins(2, VOTING_TOKEN));
    let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        config,
        Config {
            glow_token: Addr::unchecked("".to_string()),
            ve_token: Addr::unchecked("".to_string()),
            terraswap_factory: Addr::unchecked("".to_string()),
            owner: deps.api.addr_validate(TEST_CREATOR).unwrap(),
        }
    );

    let msg = ExecuteMsg::RegisterContracts {
        glow_token: VOTING_TOKEN.to_string(),
        ve_token: VE_TOKEN.to_string(),
        terraswap_factory: TERRASWAP_FACTORY.to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let config = CONFIG.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        config.glow_token,
        deps.api.addr_validate(VOTING_TOKEN).unwrap()
    );

    let state = STATE.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_validate(MOCK_CONTRACT_ADDR).unwrap(),
            total_distributed_unclaimed_fees: Uint128::zero(),
        }
    );
}

#[test]
fn fail_distribute_glow_nothing_staked() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &[]);

    let distribute_msg = ExecuteMsg::DistributeGlow {};
    let execute_res = execute(deps.as_mut(), env, info, distribute_msg);

    match execute_res {
        Err(ContractError::NothingStaked {}) => {}
        _ => panic!("DO NOT ENTER"),
    };
}

#[test]
fn fail_distribute_glow_nothing_to_distribute() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &[]);

    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(0_u128))],
        ),
        (
            &VE_TOKEN.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(100_u128))],
        ),
    ]);

    let distribute_msg = ExecuteMsg::DistributeGlow {};
    let execute_res = execute(deps.as_mut(), env, info, distribute_msg);

    match execute_res {
        Err(ContractError::NothingToDistribute {}) => {}
        _ => panic!("DO NOT ENTER"),
    };
}

#[test]
fn distribute_glow_to_voter() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let mut env = mock_env_height(0, 1000000);
    let info = mock_info(VOTING_TOKEN, &[]);

    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(10u128))],
        ),
        (
            &VE_TOKEN.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(100u128))],
        ),
    ]);

    let distribute_msg = ExecuteMsg::DistributeGlow {};
    let _execute_res = execute(deps.as_mut(), env.clone(), info, distribute_msg).unwrap();

    // Increase the clock by a week to get things going

    increase_env_time(&mut env, SECONDS_PER_WEEK);

    // Verify that the voter has a minimum balance of 10

    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::Staker {
            address: TEST_VOTER.to_string(),
            fee_start_after: None,
            fee_limit: None,
        },
    )
    .unwrap();
    let response: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        StakerResponse {
            balance: Uint128::from(100u128),
            initial_last_claimed_fee_timestamp: 0,
            last_claimed_fee_timestamp: 1000000 / SECONDS_PER_WEEK * SECONDS_PER_WEEK,
            claimable_fees_lower_bound: Uint128::from(10u128)
        }
    );

    // Try to claim

    let info = mock_info(TEST_VOTER, &[]);

    let claim_msg = ExecuteMsg::Claim { limit: None };
    let execute_res = execute(deps.as_mut(), env.clone(), info, claim_msg).unwrap();

    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(10u128),
            })
            .unwrap(),
        }))]
    )
}

#[test]
fn distribute_glow_to_voter_2() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_register_contracts(deps.as_mut());
    let mut env = mock_env_height(0, 1000000);
    let info = mock_info(VOTING_TOKEN, &[]);

    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(10u128))],
        ),
        (
            &VE_TOKEN.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(100u128))],
        ),
    ]);

    let distribute_msg = ExecuteMsg::DistributeGlow {};
    let _execute_res = execute(deps.as_mut(), env.clone(), info, distribute_msg).unwrap();

    // Increase the clock by a week to get things going

    increase_env_time(&mut env, SECONDS_PER_WEEK);

    // Verify that the voter has a minimum balance of 10

    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::Staker {
            address: TEST_VOTER.to_string(),
            fee_start_after: None,
            fee_limit: None,
        },
    )
    .unwrap();
    let response: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        StakerResponse {
            balance: Uint128::from(100u128),
            initial_last_claimed_fee_timestamp: 0,
            last_claimed_fee_timestamp: 1000000 / SECONDS_PER_WEEK * SECONDS_PER_WEEK,
            claimable_fees_lower_bound: Uint128::from(10u128)
        }
    );

    // Try to claim

    let info = mock_info(TEST_VOTER, &[]);

    let claim_msg = ExecuteMsg::Claim { limit: None };
    let execute_res = execute(deps.as_mut(), env.clone(), info, claim_msg).unwrap();

    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(10u128),
            })
            .unwrap(),
        }))]
    )
}
