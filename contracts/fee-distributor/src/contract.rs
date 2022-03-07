#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw_storage_plus::U64Key;
use terraswap::asset::{Asset, AssetInfo, PairInfo};

use crate::error::ContractError;
use crate::helpers::compute_claimable;
use crate::querier::{
    query_address_voting_balance_at_timestamp, query_total_voting_balance_at_timestamp,
};
use crate::state::{
    Config, State, CONFIG, STATE, USER_LAST_CLAIMED_FEE_TIMESTAMP, WEEKLY_TOKEN_DISTRIBUTION,
};

use cosmwasm_std::{
    attr, to_binary, Addr, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use glow_protocol::fee_distributor::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, StakerResponse, StateResponse,
};

use terraswap::querier::{query_balance, query_pair_info, query_token_balance};

use terraswap::pair::ExecuteMsg as TerraswapExecuteMsg;

pub const SECONDS_PER_WEEK: u64 = 7 * 24 * 60 * 60;
pub const DEFAULT_CLAIM_LIMIT: u32 = 20;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        glow_token: Addr::unchecked(""),
        ve_token: Addr::unchecked(""),
        terraswap_factory: Addr::unchecked(""),
        owner: info.sender,
    };

    let state = State {
        contract_addr: env.contract.address,
        total_distributed_unclaimed_fees: Uint128::zero(),
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &state)?;

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
        ExecuteMsg::RegisterContracts {
            glow_token,
            ve_token,
            terraswap_factory,
        } => register_contracts(deps, glow_token, ve_token, terraswap_factory),
        ExecuteMsg::Sweep { denom } => sweep(deps, env, denom),
        ExecuteMsg::DistributeGlow {} => distribute_glow(deps, env),
        ExecuteMsg::Claim { limit } => claim(deps, env, info, limit),
        ExecuteMsg::UpdateConfig { owner } => update_config(deps, info, owner),
    }
}

pub fn distribute_glow(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    // Get the config and mutable state
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // Get the ve token address and the timestamp of the current time
    // floored down to the nearest week.
    let ve_token_addr = &config.ve_token;
    let week_timestamp = env.block.time.seconds() / SECONDS_PER_WEEK * SECONDS_PER_WEEK;

    // Get the total voting balance
    let total_voting_balance = query_total_voting_balance_at_timestamp(
        &deps.querier,
        ve_token_addr,
        Some(week_timestamp),
    )?;

    // If nothing is staked, return an error.
    if total_voting_balance == Uint128::zero() {
        return Err(ContractError::NothingStaked {});
    }

    // Get the amount to distribute which includes the GLOW that has just been sent to the contractx
    // but subtracts the amount reserved for previous unclaimed fee distribution.
    let amount_to_distribute = query_token_balance(
        &deps.querier,
        config.glow_token,
        state.contract_addr.clone(),
    )?
    .checked_sub(state.total_distributed_unclaimed_fees)?;

    // Verify that the amount to distribute is non zero.
    if amount_to_distribute == Uint128::zero() {
        return Err(ContractError::NothingToDistribute {});
    }

    // Define the function for increment token distribution amount by
    // amount_to_distribute
    let add_to_week_token_distribution =
        |maybe_distribution: Option<Uint128>| -> StdResult<Uint128> {
            Ok(maybe_distribution.unwrap_or_default() + amount_to_distribute)
        };

    // Update WEEKLY_TOKEN_DISTRIBUTION according to the new amount_to_distribute
    WEEKLY_TOKEN_DISTRIBUTION.update(
        deps.storage,
        U64Key::from(week_timestamp),
        add_to_week_token_distribution,
    )?;

    // Save the state to increase total_distributed_unclaimed_fees
    state.total_distributed_unclaimed_fees += amount_to_distribute;
    STATE.save(deps.storage, &state)?;

    // Return with Response
    Ok(Response::default().add_attributes(vec![
        attr("action", "distribute_glow"),
        attr("glow_distributed", amount_to_distribute.to_string()),
        attr("week_timestamp", week_timestamp.to_string()),
    ]))
}

pub fn claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    limit: Option<u32>,
) -> Result<Response, ContractError> {
    // Read the conig and mutable state
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // Compute the claimable amount
    let (initial_last_claimed_fee_timestamp, last_claimed_fee_timestamp, claim_amount) =
        compute_claimable(deps.as_ref(), env, &config, &info.sender, limit, None)?;

    // Save the last_claimed_fee_timestamp to the user.
    USER_LAST_CLAIMED_FEE_TIMESTAMP.save(
        deps.storage,
        info.sender.clone(),
        &last_claimed_fee_timestamp,
    )?;

    // Decrease total_distributed_unclaimed fee by the claimed amount.
    state.total_distributed_unclaimed_fees -= claim_amount;
    STATE.save(deps.storage, &state)?;

    // Return with a message to send "claim_amount" GLOW to the calling user.
    Ok(Response::default()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.glow_token.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount: claim_amount,
            })?,
        }))
        .add_attributes(vec![
            attr("action", "claim"),
            attr("claimed_amount", claim_amount.to_string()),
            attr(
                "initial_last_claimed_fee_timestamp",
                initial_last_claimed_fee_timestamp.to_string(),
            ),
            attr(
                "last_claimed_fee_timestamp",
                last_claimed_fee_timestamp.to_string(),
            ),
        ]))
}

/// Register the addresses of the glow_token, ve_token, and terraswap_factory contracts
pub fn register_contracts(
    deps: DepsMut,
    glow_token: String,
    ve_token: String,
    terraswap_factory: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if config.glow_token != Addr::unchecked("") {
        return Err(ContractError::Unauthorized {});
    }

    config.glow_token = deps.api.addr_validate(&glow_token)?;
    config.ve_token = deps.api.addr_validate(&ve_token)?;
    config.terraswap_factory = deps.api.addr_validate(&terraswap_factory)?;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

/// Sweep
/// Anyone can execute sweep function to swap
/// asset native denom => GLOW token
pub fn sweep(deps: DepsMut, env: Env, denom: String) -> Result<Response, ContractError> {
    // Read the config, glow_token, and terraswap_factory_addr
    let config = CONFIG.load(deps.storage)?;
    let glow_token = config.glow_token;
    let terraswap_factory_addr = config.terraswap_factory;

    // Get the pair info
    let pair_info: PairInfo = query_pair_info(
        &deps.querier,
        terraswap_factory_addr,
        &[
            AssetInfo::NativeToken {
                denom: denom.to_string(),
            },
            AssetInfo::Token {
                contract_addr: glow_token.to_string(),
            },
        ],
    )?;

    // Sweep the entire balance worth of the denom to glow
    let amount = query_balance(&deps.querier, env.contract.address, denom.to_string())?;
    let swap_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: denom.to_string(),
        },
        amount,
    };

    // Deduct tax first
    let amount = (swap_asset.deduct_tax(&deps.querier)?).amount;

    // Response which sweeps all the contracts UST for GLOW
    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pair_info.contract_addr,
            msg: to_binary(&TerraswapExecuteMsg::Swap {
                offer_asset: Asset {
                    amount,
                    ..swap_asset
                },
                max_spread: None,
                belief_price: None,
                to: None,
            })?,
            funds: vec![Coin {
                denom: denom.to_string(),
                amount,
            }],
        }))
        .add_attributes(vec![
            attr("action", "sweep"),
            attr(
                "collected_rewards",
                format!("{:?}{:?}", amount.to_string(), denom),
            ),
        ]))
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
) -> Result<Response, ContractError> {
    let api = deps.api;
    CONFIG.update(deps.storage, |mut config| {
        if config.owner != info.sender {
            return Err(ContractError::Unauthorized {});
        }

        if let Some(owner) = owner {
            config.owner = api.addr_validate(&owner)?;
        }

        Ok(config)
    })?;

    Ok(Response::new().add_attributes(vec![("action", "update_config")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
        QueryMsg::State {} => Ok(to_binary(&query_state(deps)?)?),
        QueryMsg::Staker {
            address,
            fee_limit,
            fee_start_after,
        } => Ok(to_binary(&query_staker(
            deps,
            env,
            address,
            fee_limit,
            fee_start_after,
        )?)?),
    }
}

fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        glow_token: config.glow_token.to_string(),
        ve_token: config.ve_token.to_string(),
        terraswap_factory: config.terraswap_factory.to_string(),
    })
}

fn query_state(deps: Deps) -> Result<StateResponse, ContractError> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {
        contract_addr: state.contract_addr.to_string(),
        total_distributed_unclaimed_fees: state.total_distributed_unclaimed_fees,
    })
}

fn query_staker(
    deps: Deps,
    env: Env,
    address: String,
    fee_limit: Option<u32>,
    fee_start_after: Option<u64>,
) -> Result<StakerResponse, ContractError> {
    // Validate the user's address and read the config
    let address = deps.api.addr_validate(&address)?;
    let config = CONFIG.load(deps.storage)?;

    // Get the last_claimed_timestamp, and calculate a lower bound on the user's
    // claimable fees.
    let (initial_last_claimed_fee_timestamp, last_claimed_fee_timestamp, claim_amount) =
        compute_claimable(deps, env, &config, &address, fee_limit, fee_start_after)?;

    // Get the user's voting balance just to add it as more data to the response.
    let balance =
        query_address_voting_balance_at_timestamp(&deps.querier, &config.ve_token, None, &address)?;

    Ok(StakerResponse {
        balance,
        initial_last_claimed_fee_timestamp,
        last_claimed_fee_timestamp,
        claimable_fees_lower_bound: claim_amount,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
