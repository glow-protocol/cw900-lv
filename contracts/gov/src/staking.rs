use crate::state::{
    bank_read, config_read, poll_read, poll_voter_store, state_read, state_store, Config, Poll,
    State, TokenManager,
};

use cosmwasm_std::{
    to_binary, Addr, CanonicalAddr, CosmosMsg, Deps, DepsMut, MessageInfo, Response, StdResult,
    Storage, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use glow_protocol::gov::{PollStatus, StakerResponse};
use terraswap::querier::query_token_balance;

pub fn query_staker(deps: Deps, address: String) -> StdResult<StakerResponse> {
    let addr_raw = deps.api.addr_canonicalize(&address).unwrap();
    let config: Config = config_read(deps.storage).load()?;
    let state: State = state_read(deps.storage).load()?;
    let mut token_manager = bank_read(deps.storage)
        .may_load(addr_raw.as_slice())?
        .unwrap_or_default();

    // filter out not in-progress polls
    token_manager.locked_balance.retain(|(poll_id, _)| {
        let poll: Poll = poll_read(deps.storage)
            .load(&poll_id.to_be_bytes())
            .unwrap();

        poll.status == PollStatus::InProgress
    });

    let total_balance = query_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.glow_token)?,
        deps.api.addr_humanize(&state.contract_addr)?,
    )?
    .checked_sub(state.total_deposit)?;

    Ok(StakerResponse {
        balance: if !state.total_share.is_zero() {
            token_manager
                .share
                .multiply_ratio(total_balance, state.total_share)
        } else {
            Uint128::zero()
        },
        share: token_manager.share,
        locked_balance: token_manager.locked_balance,
    })
}
