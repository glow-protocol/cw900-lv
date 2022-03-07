# Vote Escrowed Token

The Vote Escrowed Token contract contains logic for Glow Token (GLOW) staking.

veGLOW cannot be transferred. The only way to obtain veGLOW is by locking veGLOW. The maximum lock time is one year. One GLOW locked for 1 year provides an initial balance of one veGLOW.

A user's veGLOW balance decays quadratically with respect to time until the end lock time is reached.

## User Actions

The available user actions are:
- **CreateLock**. Create a lock when you don't already have one. Specify the `end_lock_time` as a unix timestamp and the amount to lock.
- **IncreaseAmount**. Increase the amount of your existing lock. Resets the `start_lock_time`.
- **IncreaseEndLockTime**. Increase the end time of your existing lock. Resets the `start_lock_time`.
- **Withdraw**. If your lock is expired, withdraw the entire `deposited_amount` and void the lock. If the lock is not expired, withdraw all funds available to withdraw and reset the `start_lock_time`.

## Implementation Details

A user's voting power decreases quadratically since the moment of the lock. So does the total voting power.

Each user's lock (`UserLockedBalance`) is represented by its `start_lock_time`, `end_lock_time`, and `deposited_amount`.

From these values we can calculate the three coefficients which represent the quadratic equation corresponding to the user's voting power balance.

Intuitively, the user's voting power is quadratic because both the user lock's locked_amount and remaining time are decreasing linearly with time. Squaring these results in a quadratic.

The state stores `total_balance_coefficients` which store the three coefficients representing the quadratic equation corresponding to the sum of all the user voting power balances.

We prefer this form when representing the sum of all the user voting power balances because we can easily add and subtract quadratic equations from one another using these coefficients.

So whenever a user modifies a lock, we subtract the quadratic coefficients corresponding to the lock to be replaced from the state, and add the quadratic coefficients corresponding to the new lock.

Then we schedule to subtract the quadratic coefficients of the new lock at its expiry time.

In order to reduce the number of times for which changes can be scheduled, we round all end lock times to the nearest week.

### Lock States

A lock is always in one of four states:
- Active: This means a lock with an `end_lock_time` in the future. All locks with an `end_lock_time` in the future have a positive `start_lock_time`, `deposited_amount`, and `timestamp`.
- Expired: This means a lock with an `end_lock_time` in the past or at the current timestamp. All expired locks have a positive `start_lock_time`, `deposited_amount`, and `timestamp`.
- Void: This represents the lack of a lock following a full withdrawal. `start_lock_time`, `end_lock_time`, and `deposited_amount` are all 0. `timestamp` is positive.
- Undefined: This represents the lack of a lock for those who have never interacted with the veGLOW contract. `start_lock_time`, `end_lock_time`, `deposited_amount`, and `timestamp` are all 0.
### Start Lock Time

`start_lock_time` and `end_lock_time` are used to calculate the percentage of `deposited_amount` a locker has available to withdraw.

When time equals `start_lock_time` there is nothing available to withdraw, when time equals `end_lock_time` everything is available to withdraw, and when time equals `(start_lock_time + end_lock_time) / 2` half of `deposited_amount` is available to withdraw.

This means that half way into a one year lock. a locker will be able to withdraw half of the funds they locked up.

"Resetting `start_lock_time`" effectively means relocking up the portion of `deposited_amount` which has become available to unlock. 

### Lock Up Propogation

Upon making changes to a lock, the changes to voting power only become visible in the second following the timestamp at which the changes were made.

### Queries

The main queries are:
- `State { timestamp: Option<u64> }`. Read the `total_deposited_amount` and `total_balance` at a given timestamp. If no timestamp is specified, use the current timestamp. `total_balance` refers to the total voting power.
- `Staker { address: String, timestamp: Option<u64> }`. Read the `deposited_amount`, `locked_amount`, and `balance` of a user at a given timestamp. If no timestamp is specified, use the current timestamp. `balance` refers to the user's voting power, and `deposited_amount - locked_amount` gives the amount available to withdraw. 

## References

- This implementation was heavily inspired by veCRV! The main difference is that veCRV locks operate with a cliff unlock, while veGLOW operates with a linear unlock. https://curve.readthedocs.io/dao-vecrv.html

## Linear Unlock vs Cliff Unlock

A natural question is to why veGLOW switched to a linear unlock instead of going for a linear unlock like veCRV.

Some of the benefits include:
- Locking up doesn't require as much of a commitment because half way into the lock you will have gotten half of your funds back.
- In the veCRV model unlocks happen on weekly intervals where everybody unlocks at once. This can lead to a rush to sell in an attempt to get out before the sell pressure materializes. In the linear unlock model this isn't a problem because you can unlock your a linear portion of your deposit at any time.
- You can think of a linear unlock as being equivalent to a bunch of tiny locks with end lock times evenly distributed between the start lock time and the end lock time. This is a more natural way of locking than having to pick a single end lock time and put all of your eggs into that basket.
