# Glow Protocol Contracts

This monorepository contains the source code for the core smart contracts implementing Glow Protocol on the [Terra](https://terra.money) blockchain.

You can find information about the architecture, usage, and function of the smart contracts on the official Glow documentation [site](https://docs.glowyield.com/).

### Dependencies

Glow depends on [Anchor Protocol](https://anchorprotocol.com) and [Terraswap](https://terraswap.io) and uses its [implementation](https://github.com/terraswap/terraswap) of the CW20 token specification.

## Contracts

| Contract                                            | Reference                                               | Description                                                                                                          |
| --------------------------------------------------- |---------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------|
| [`lotto`](./contracts/lotto)  | [doc](https://docs.glowyield.com/contracts/lotto)       | Glow Lotto, a prize-linked savings accounts built on top of Anchor money-market                                      |
| [`community`](./contracts/community) | [doc](https://docs.glowyield.com/contracts/community)   | Manages the Glow Ecosystem Fund (GEF)                                                                                |
| [`gov`](./contracts/gov)              | [doc](https://docs.glowyield.com/contracts/gov)         | Allows Glow contracts to be controlled by decentralized governance and distributes protocol revenues to GLOW stakers |
| [`staking`](./contracts/staking)      | [doc](https://docs.glowyield.com/contracts/staking)     | Distributes GLOW rewards to to LP stakers                                                                            |
| [`distibutor`](./contracts/distributor)      | [doc](https://docs.glowyield.com/contracts/distributor) | Distributes GLOW rewards to users of Glow protocol based on a dynamic emission rate algorithm                        |
| [`airdrop`](./contracts/airdrop)      | [doc](https://docs.glowyield.com/contracts/airdrop)     | Distributes GLOW to selected addresses at the token genesis event                                                    |
| [`vesting`](./contracts/vesting)      | [doc](https://docs.glowyield.com/contracts/vesting)               | Distributes GLOW over time to founding team, investors and contributors                                              |

## Development

### Environment Setup

- Rust v1.52.0+
- `wasm32-unknown-unknown` target
- Docker

1. Install `rustup` via https://rustup.rs/

2. Run the following:

```sh
rustup default stable
rustup target add wasm32-unknown-unknown
```

3. Make sure [Docker](https://www.docker.com/) is installed

### Unit / Integration Tests

Each contract contains Rust unit and integration tests embedded within the contract source directories. You can run:

```sh
cargo test
cargo unit-test
cargo integration-test
```

### Compiling

After making sure tests pass, you can compile each contract with the following:

```sh
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ../../target/wasm32-unknown-unknown/release/cw1_subkeys.wasm .
ls -l cw1_subkeys.wasm
sha256sum cw1_subkeys.wasm
```

#### Production

For production builds, run the following:

```sh
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.11.5
```

This performs several optimizations which can significantly reduce the final size of the contract binaries, which will be available inside the `artifacts/` directory.

## License

Copyright 2021 Glow Protocol Ltd.

Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0. Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.

See the License for the specific language governing permissions and limitations under the License.
