# Fee Distribution

The Fee Distributor Contract contains logic for distributing GLOW among veGLOW holders.

The Fee Distributor Contract keeps a balance of GLOW tokens, which it uses to reward stakers with funds it receives from trading fees sent by the Glow Collector. This balance is separate from the Community Pool, which is held by the Community contract (owned by the Gov contract).

The fee distributor contract supports `Claim`, `DistributeGlow`, and `Sweep` functions.
- `DistributeGlow` distributes Glow among veGLOW holders.
- `Claim` lets a user collect the fees that have been collected for them.
- `Sweep` converts Terra native tokens held by the contract to `GLOW`

## Main Execute Messages

### DistributeGlow

Distribute glow can be called to distribute all available glow in the gov contract to veGlow holders. Available glow means the Glow balance of the gov contract minus the amount reserved for polls and reserved for past glow distributions. 

Glow distribution works by making use of the `WEEKLY_TOKEN_DISTRIBUTION` map.

Token distribution takes place in weekly intervals. Upon calling `DistributeGlow`, the corresponding Glow gets added to the `WEEKLY_TOKEN_DISTRIBUTION` with the timestamp of `env.block.time.seconds() / SECONDS_PER_WEEK * SECONDS_PER_WEEK` (the current timestamp rounded down to the nearest week).

When calling `distribute_glow`, the corresponding amount is added to `total_distributed_unclaimed_fees`.

### Claim

The claim function works by taking a range from `last_claimed_fee_timestamp + SECONDS_PER_WEEK` to `end.block.time.seconds() / SECONDS_PER_WEEK * SECONDS_PER_WEEK - SECONDS_PER_WEEK` over the `WEEKLY_TOKEN_DISTRIBUTION` map.

For each distribution, increment `claim_amount` by the `distributed_amount` for that week times the ratio of `user_voting_balance / total_voting_balance`. `user_voting_balance` and `total_voting_balance` are the corresponding `veGLOW` balances at the time of the start of the corresponding weekly distribution.

Upon claming:
-  the `last_claimed_fee_timestamp` for the corresponding user will be set to the timestamp of the last distribution that was claimed in order to prevent users from double claiming the same distribution.
- state will get updated to reduce `total_distributed_pending_claiming` by `claim_amount`
- `claim_amount` of Glow will get sent to the claimer.


### Sweep

The `Sweep` function was added to the fee distributor contract, but it doesn't perform the glow distribution. In order to sweep and distribute the corresponding glow to stakers, you must call `Sweep` and then call `DistributeGlow` afterwards.

## Main Queries Messages
### Staker

A `Staker` query is exposed for getting information about how much is available for a user to claim. It returns the claimable amount, information for pagination, and the total voting power at the time of the query.

### State

A `State` query is exposed for getting the `total_distributed_unclaimed_fees`.