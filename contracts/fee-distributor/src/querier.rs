use cosmwasm_std::Uint128;
use cosmwasm_std::{to_binary, Addr, QuerierWrapper, QueryRequest, StdResult, WasmQuery};

use glow_protocol::ve_token::{QueryMsg as VEQueryMessage, StakerResponse, StateResponse};

pub fn query_address_voting_balance_at_timestamp(
    querier: &QuerierWrapper,
    ve_addr: &Addr,
    timestamp: Option<u64>,
    address: &Addr,
) -> StdResult<Uint128> {
    let balance: StdResult<StakerResponse> = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: ve_addr.to_string(),
        msg: to_binary(&VEQueryMessage::Staker {
            address: address.to_string(),
            timestamp,
        })?,
    }));

    Ok(balance.map_or(Uint128::zero(), |s| s.balance))
}

pub fn query_total_voting_balance_at_timestamp(
    querier: &QuerierWrapper,
    ve_addr: &Addr,
    timestamp: Option<u64>,
) -> StdResult<Uint128> {
    let total_supply: StdResult<StateResponse> =
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: ve_addr.to_string(),
            msg: to_binary(&VEQueryMessage::State { timestamp })?,
        }));

    let res = total_supply.map_or(Uint128::zero(), |t| t.total_balance);

    Ok(res)
}
