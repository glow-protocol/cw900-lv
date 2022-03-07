use cosmwasm_std::Uint128;
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    Withdraw {},
    Checkpoint {},
    IncreaseEndLockTime {
        // unlock_week specifies the week at which to unlock
        // in units of weeks since the epoch
        end_lock_time: u64,
    },
    RegisterContracts {
        cw20_address: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// StakeVotingTokens a user can stake their mirror token to receive rewards
    /// or do vote on polls
    CreateLock {
        // unlock_week specifies the week at which to unlock
        // in units of weeks since the epoch
        end_lock_time: u64,
    },
    IncreaseLockAmount {},
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {
        timestamp: Option<u64>,
    },
    Staker {
        address: String,
        timestamp: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub cw20_address: String,
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub total_deposited_amount: Uint128,
    pub total_locked_amount: Uint128,
    pub total_balance: Uint128,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct StakerResponse {
    pub deposited_amount: Uint128,
    pub locked_amount: Uint128,
    pub balance: Uint128,
}
