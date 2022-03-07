use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map, U64Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");

pub const WEEKLY_TOKEN_DISTRIBUTION: Map<U64Key, Uint128> = Map::new("distributed_tokens");

pub const USER_LAST_CLAIMED_FEE_TIMESTAMP: Map<Addr, u64> = Map::new("user_last_claimed_fee");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub glow_token: Addr,
    pub ve_token: Addr,
    pub terraswap_factory: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub contract_addr: Addr,
    pub total_distributed_unclaimed_fees: Uint128,
}
