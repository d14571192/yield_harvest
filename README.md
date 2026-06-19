# yield_harvest

## Project Title
yield_harvest

## Project Description
yield_harvest is a Soroban smart contract that implements a simplified DeFi
yield aggregator. Users deposit principal into a shared on-chain strategy; the
contract accrues yield linearly over time based on a configurable rate and
compounding period. Depositors can `harvest` accrued yield at any time,
`compound` it back into their principal for a compounding effect, or
`withdraw` their principal together with any remaining yield. The goal of
the project is to demonstrate the core accounting and authorization patterns
of a yield-bearing vault on Stellar in a small, readable contract.

## Project Vision
The long-term vision is to give Stellar builders a tiny, auditable reference
implementation of a yield aggregator that can be extended into a full-featured
DeFi primitive: multi-strategy routing (lending, LP, liquid staking), fee
accrual for the protocol, on-chain strategy migration, and a front-end that
abstracts the auto-compounding behavior away from end users. Even this MVP
shows the "deposit, wait, harvest" loop that underpins most CeDeFi and DeFi
yield products.

## Key Features
- **Single-call deposit** — `deposit(user, amount)` opens or grows a
  position and updates the strategy-wide principal counter.
- **Harvest accrued yield** — `harvest(user)` settles and returns the
  caller's pending yield without touching the principal.
- **Manual compounding** — `compound(user)` rolls pending yield back into
  the user's principal, effectively boosting the APY through more frequent
  capital reinvestment.
- **Flexible withdrawal** — `withdraw(user, amount)` returns the requested
  principal plus any remaining unharvested yield in a single call.
- **Read-only accounting** — `pending_yield(user)` and `total_principal()`
  expose the on-chain state for wallets, dashboards, and indexers without
  changing it.
- **Owner-govened rate updates** — the strategy owner can call
  `set_rate(new_rate_bps)`; the contract settles all yield at the old rate
  before applying the new one, preventing retroactive re-pricing.
- **Authorization everywhere** — every state-changing function calls
  `require_auth()` on the affected user (or the owner) before touching
  storage.

## Contract

- **Network:** Stellar Testnet (Public)
- **Scope:** finance dApp — see `contracts/yield_harvest/src/lib.rs` for the full yield_harvest business logic.
- **Functions exposed:** see `Key Features` above and the `pub fn` list in `lib.rs`.
- **Contract ID:** `<to be deployed on Stellar Testnet>`
- **Explorer template:** `https://stellar.expert/explorer/testnet`
- **Screenshot of deployed contract on Stellar Expert:**
  `_(Screenshot of the contract page on Stellar Expert will appear here after deploy.)_`


## Future Scope
- Pluggable strategy adapters that route deposits to different yield sources
  (lending pools, AMM LP, liquid staking) and report yield back to the
  aggregator.
- On-chain strategy migration with a timelock so users can exit before a
  migration executes.
- A protocol fee (configurable in basis points) taken from each harvest and
  sent to a treasury address, mirroring mainstream DeFi yield aggregators.
- A minimal web front-end built with the Stellar SDK + Freighter that lets
  users connect a wallet, deposit, view pending yield, and harvest in a
  single click.
- Comprehensive unit tests against `soroban-sdk` testutils, including
  multi-user yield accounting and time-warp scenarios.
- Audit pass and a path to Mainnet once a stable, audited Soroban token
  integration is wired in (this MVP intentionally performs no real asset
  transfer).

## Profile

- **Name:** <!-- Fill github name -->
- **Project:** `yield_harvest` (finance)
- **Built with:** Soroban SDK 25, Rust, Stellar Testnet
