use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    RegisterContracts {
        glow_token: String,
        ve_token: String,
        terraswap_factory: String,
    },
    /// Public Message
    Sweep {
        denom: String,
    },
    Claim {
        limit: Option<u32>,
    },
    DistributeGlow {},
    UpdateConfig {
        owner: Option<String>,
    },
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    Staker {
        address: String,
        fee_limit: Option<u32>,
        fee_start_after: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub glow_token: String,
    pub ve_token: String,
    pub terraswap_factory: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub contract_addr: String,
    pub total_distributed_unclaimed_fees: Uint128,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct StakerResponse {
    pub balance: Uint128,
    pub initial_last_claimed_fee_timestamp: u64,
    pub last_claimed_fee_timestamp: u64,
    pub claimable_fees_lower_bound: Uint128,
}
