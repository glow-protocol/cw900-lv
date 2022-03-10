use core::fmt;
use std::convert::TryFrom;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use cosmwasm_std::{Addr, Decimal256, Uint128, Uint256};
use cw_storage_plus::{Item, Map, SnapshotItem, SnapshotMap, U64Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SECONDS_PER_WEEK: u64 = 7 * 24 * 60 * 60; // Order of 10 ** 6
pub const MAX_WEEKS: u64 = 52;
pub const MAX_SECONDS: u64 = MAX_WEEKS * SECONDS_PER_WEEK; // Order of 10 ** 8
pub const VOTING_POWER_CONSTANT_DIVISOR: u64 = MAX_SECONDS;

pub const CONFIG: Item<Config> = Item::new("config");
// pub const STATE: Item<State> = Item::new("state");
pub const COEFFICIENT_CHANGES: Map<U64Key, QuadraticEquationCoefficients> =
    Map::new("coefficient_changes");

pub const USER_LOCKED_BALANCES: SnapshotMap<&Addr, UserLockedBalance> = SnapshotMap::new(
    "user_locked_balance",
    "user_locked_balance__checkpoint",
    "user_locked_balance__changelog",
    cw_storage_plus::Strategy::EveryBlock,
);

pub const STATE: SnapshotItem<State> = SnapshotItem::new(
    "state",
    "state__checkpoint",
    "state__changelog",
    cw_storage_plus::Strategy::EveryBlock,
);
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct QuadraticEquationCoefficients {
    pub quad_coefficient: Decimal256,
    pub linear_coefficient: Decimal256,
    pub constant_coefficient: Decimal256,
}

impl QuadraticEquationCoefficients {
    pub fn evaluate_voting_power_at_timestamp(&self, timestamp: u64) -> Uint128 {
        Uint128::try_from(
            ((
                // Floor
                Uint256::from(1u128) * self.constant_coefficient
                // Floor
                + Uint256::from(timestamp) * Uint256::from(timestamp) * self.quad_coefficient)
                // Subtracts a truncated value value
                .checked_sub(Uint256::from(timestamp) * self.linear_coefficient))
            // In the event of an underflow
            // which can happen because of truncation
            // default to 0
            .unwrap_or_default()
            // Scales everything down by VOTING_POWER_CONSTANT_DIVISOR
                / Uint256::from(VOTING_POWER_CONSTANT_DIVISOR),
        )
        .unwrap()
    }

    // Notice that we can also express rla as a linear function and that:
    // - the linear coefficient of this function is the negative quadratic coefficient for vp
    // - the constant coefficient of this function is the negative linear coefficient over two for vp
    // This means we can calculate the corresponding locked amount without storing more coefficients separately!
    pub fn evaluate_locked_balance_at_timestamp(&self, timestamp: u64) -> Uint128 {
        Uint128::try_from(
            // Floor
            (Uint256::from(1u128) * self.linear_coefficient / Uint256::from(2u128))
                // Subtracts a truncated value value
                .checked_sub(Uint256::from(timestamp) * self.quad_coefficient)
                // In the event of an underflow
                // which can happen because of truncation
                // default to 0
                .unwrap_or_default(),
        )
        .unwrap()
    }
}

// Implement Display in order to make testing easier.
impl fmt::Display for QuadraticEquationCoefficients {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Quad_coefficient: {}, Linear_coefficient: {}, Constant_coefficient: {}",
            self.quad_coefficient, self.linear_coefficient, self.constant_coefficient
        )
    }
}

impl Add for QuadraticEquationCoefficients {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            quad_coefficient: self.quad_coefficient + other.quad_coefficient,
            linear_coefficient: self.linear_coefficient + other.linear_coefficient,
            constant_coefficient: self.constant_coefficient + other.constant_coefficient,
        }
    }
}

impl AddAssign for QuadraticEquationCoefficients {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            quad_coefficient: self.quad_coefficient + other.quad_coefficient,
            linear_coefficient: self.linear_coefficient + other.linear_coefficient,
            constant_coefficient: self.constant_coefficient + other.constant_coefficient,
        }
    }
}

impl Sub for QuadraticEquationCoefficients {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            quad_coefficient: self.quad_coefficient - other.quad_coefficient,
            linear_coefficient: self.linear_coefficient - other.linear_coefficient,
            constant_coefficient: self.constant_coefficient - other.constant_coefficient,
        }
    }
}

impl SubAssign for QuadraticEquationCoefficients {
    fn sub_assign(&mut self, other: Self) {
        *self = Self {
            quad_coefficient: self.quad_coefficient - other.quad_coefficient,
            linear_coefficient: self.linear_coefficient - other.linear_coefficient,
            constant_coefficient: self.constant_coefficient - other.constant_coefficient,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserLockedBalance {
    // Locked balance info
    pub deposited_amount: Uint128,
    /// On the order of 10 ** 9
    pub end_lock_time: u64,
    /// On the order of 10 ** 9
    pub start_lock_time: u64,
    // History tracking info
    pub timestamp: u64,
}

impl UserLockedBalance {
    /// Return whether or not a lock exists. If a lock exists, it is not void or undefined.
    /// void locks are used to represent the lack of a lock rather than an option type.
    /// This makes the math much easier than it would be when dealing with option types.
    pub fn exists(&self) -> bool {
        if self.deposited_amount == Uint128::zero()
            || self.end_lock_time == 0
            || self.start_lock_time == 0
        {
            if !(self.deposited_amount == Uint128::zero()
                && self.end_lock_time == 0
                && self.start_lock_time == 0)
            {
                panic!("All or nothing. This should never happen.");
            }
            false
        } else {
            true
        }
    }

    /// Returns whether or not a lock is void or undefined.
    /// Void locks are those with a timestamp but all other values 0.
    /// Undefined locks are those with all values 0.
    pub fn is_void_or_undefined(&self) -> bool {
        !self.exists()
    }

    /// Create a void lock with a timestamp
    pub fn void_lock_with_timestamp(timestamp: u64) -> Self {
        UserLockedBalance {
            deposited_amount: Uint128::zero(),
            end_lock_time: 0,
            start_lock_time: 0,
            timestamp,
        }
    }

    /// Return whether or not a lock is expired at a given timestamp.
    /// When the timestamp equals the end_lock_time, the lock is expired.
    pub fn expired_at_timestamp(&self, timestamp: u64) -> bool {
        self.end_lock_time <= timestamp
    }

    /// Return the duration of the lock upon creation
    fn initial_lock_duration(&self) -> u64 {
        // This is always positive and can't be zero
        self.end_lock_time - self.start_lock_time
    }

    /// Must be called with a timestamp after or equaling start lock time
    fn elapsed_lock_time_at_timestamp(&self, timestamp: u64) -> u64 {
        // This is only ever called when LockedBalance is valid.
        timestamp - self.start_lock_time
    }

    /// Get the remaining locked_amount for a point at a given timestamp
    /// At start_lock_time time, the locked amount equals the deposited amount
    /// At end_lock_time time, the locked amount is 0
    pub fn locked_amount_at_timestamp(&self, timestamp: u64) -> Uint128 {
        if self.is_void_or_undefined() || self.expired_at_timestamp(timestamp) {
            return Uint128::zero();
        }

        // Doing subtraction from deposited_amount in order to make sure we overestimate locked amount
        // instead of underestimating it.
        Uint128::try_from(
            Uint256::from(self.deposited_amount)
                - Uint256::from(self.deposited_amount)
                    * Decimal256::from_ratio(
                        Uint128::from(self.elapsed_lock_time_at_timestamp(timestamp)),
                        // Denominator is always positive
                        Uint128::from(self.initial_lock_duration()),
                    ),
        )
        .unwrap()
    }

    // Get the voting power for a point at a given timestamp
    pub fn voting_power_at_timestamp(&self, timestamp: u64) -> Uint128 {
        if self.is_void_or_undefined() || self.expired_at_timestamp(timestamp) {
            return Uint128::zero();
        }

        // Should always be the same as this, but because of rounding/truncation
        // it will sometimes be off by a little bit.
        // self.locked_amount_at_timestamp(timestamp)
        //     * Uint128::from(self.remaining_lock_time_at_timestamp(timestamp))
        //     / Uint128::from(VOTING_POWER_CONSTANT_DIVISOR)

        self.voting_power_coefficients()
            .evaluate_voting_power_at_timestamp(timestamp)
    }

    // The following functions are for specifying the coefficients
    // of the quadratic function specifying the voting power for a given locked balance

    // The formula is:
    // voting_power = remaining_locked_amount * remaining_lock_time / voting_power_constant_divisor
    // where remaining_locked_amount = deposited_amount * remaining_lock_time / (end_lock_time - start_lock_time)

    // But we wait until evaluating the quadratic coefficients to divide by voting_power_constant_divisor
    // This is to increase the sig figs of the quadratic coefficients
    // Also, rla is calculated as da * rlt / (elt - slt) instead of da - da * (t - slt) / (elt - slt)
    // as is done in the locked_amount function, but this is fine for a voting power calculation.

    // i.e.
    // vp = rla * rlt
    // rla = da * rlt / (elt - slt)
    // => vp = da / (elt - slt) * (elt - t)^2
    // = da / (elt - slt) * t^2
    // - 2 * elt * da / (elt - slt) * t
    // + elt^2 * da / (elt - slt)

    // Notice that we can also express rla as a linear function:
    // and that
    // - the linear coefficient of this function is the negative quadratic coefficient for vp
    // - the constant coefficient of this function is the negative linear coefficient over two for vp
    // This means we can calculate the corresponding locked amount without storing more coefficients separately!

    // rla is da * rlt / (elt - slt)
    // da * (elt - t) / (elt - slt)
    // da * elt / (elt - slt)
    // - da / (elt - slt) * t

    fn voting_power_constant_coefficient(&self) -> Decimal256 {
        if self.is_void_or_undefined() {
            return Decimal256::zero();
        }

        // First do all multiplications, then divisions
        Decimal256::from_ratio(
            Uint128::from(self.end_lock_time)
                * Uint128::from(self.end_lock_time)
                * self.deposited_amount,
            // Denominator is always positive
            Uint128::from(self.initial_lock_duration()),
        )
    }

    fn voting_power_linear_coefficient(&self) -> Decimal256 {
        if self.is_void_or_undefined() {
            return Decimal256::zero();
        }

        // First do all multiplications, then divisions
        Decimal256::from_ratio(
            Uint128::from(2 * self.end_lock_time) * self.deposited_amount,
            // Denominator is always positive
            Uint128::from(self.initial_lock_duration()),
        )
    }

    fn voting_power_quad_coefficient(&self) -> Decimal256 {
        if self.is_void_or_undefined() {
            return Decimal256::zero();
        }

        // First do all multiplications, then divisions
        Decimal256::from_ratio(
            self.deposited_amount,
            // Denominator is always positive
            Uint128::from(self.initial_lock_duration()),
        )
    }

    pub fn voting_power_coefficients(&self) -> QuadraticEquationCoefficients {
        if self.is_void_or_undefined() {
            return QuadraticEquationCoefficients::default();
        }

        QuadraticEquationCoefficients {
            constant_coefficient: self.voting_power_constant_coefficient(),
            linear_coefficient: self.voting_power_linear_coefficient(),
            quad_coefficient: self.voting_power_quad_coefficient(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub cw20_address: Option<Addr>,
    pub owner: Addr,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Total voting power function definition
    pub voting_power_coefficients: QuadraticEquationCoefficients,
    /// Track total_deposit amount
    pub total_deposit: Uint128,
    /// History tracking
    pub timestamp: u64,
}
