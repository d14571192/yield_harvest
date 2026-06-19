#![no_std]

//! yield_harvest — DeFi yield aggregator
//!
//! A Soroban smart contract that allows users to deposit principal into a
//! yield-bearing strategy. Yield is accrued linearly over time based on a
//! configurable rate (in basis points) and a configurable compounding period
//! (in seconds). Users can call `harvest` at any time to claim accumulated
//! yield, `compound` to roll yield back into their principal, or `withdraw`
//! to exit the strategy with their principal plus any remaining yield.
//!
//! This contract intentionally performs no real XLM or token transfer — it
//! is a self-contained accounting demo of the yield aggregator pattern.

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol};

/// Storage key for the strategy owner / admin.
const OWNER: Symbol = symbol_short!("OWNER");
/// Storage key for the yield rate in basis points (1 bps = 0.01%) per
/// compounding period.
const RATE_BPS: Symbol = symbol_short!("RATE_BPS");
/// Storage key for the length of one compounding period, in seconds.
const PERIOD: Symbol = symbol_short!("PERIOD");
/// Storage key for the strategy-wide last compound timestamp.
const LAST_COMPOUND: Symbol = symbol_short!("LAST_CMP");
/// Storage key for the total principal currently deposited in the strategy.
const TOTAL_PRINCIPAL: Symbol = symbol_short!("TOT_PRIN");
/// Storage key flag indicating whether the strategy has been initialized.
const INIT: Symbol = symbol_short!("INIT");

/// Per-user position stored in contract storage.
/// FIX: Added #[contracttype] so Soroban SDK can serialize/deserialize
/// this struct to/from storage Val.
#[contracttype]
#[derive(Clone)]
pub struct Position {
    /// Principal currently deposited by the user.
    pub principal: u64,
    /// Yield that was accrued but not yet paid out / compounded.
    pub yield_debt: u64,
    /// Ledger timestamp of the last yield settlement for this user.
    pub last_update: u64,
}

/// Storage key type for per-user positions.
/// FIX: Using a proper #[contracttype] enum as storage key instead of
/// a raw tuple (Symbol, Address), which is not supported as a storage key.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Position(Address),
}

#[contract]
pub struct YieldHarvest;

#[contractimpl]
impl YieldHarvest {
    /// Initialize the strategy. Can only be called once.
    ///
    /// * `owner`        — admin address that may update the yield rate.
    /// * `rate_bps`     — yield rate in basis points applied each
    ///                    compounding period (e.g. `50` = 0.50% / period).
    /// * `compound_period` — length of one compounding period, in seconds.
    pub fn initialize(env: Env, owner: Address, rate_bps: u64, compound_period: u64) {
        if env.storage().instance().has(&INIT) {
            panic!("already initialized");
        }
        owner.require_auth();

        if rate_bps > 10_000 {
            panic!("rate_bps exceeds 100% per period");
        }
        if compound_period == 0 {
            panic!("compound_period must be > 0");
        }

        env.storage().instance().set(&OWNER, &owner);
        env.storage().instance().set(&RATE_BPS, &rate_bps);
        env.storage().instance().set(&PERIOD, &compound_period);
        env.storage().instance().set(&LAST_COMPOUND, &env.ledger().timestamp());
        env.storage().instance().set(&TOTAL_PRINCIPAL, &0u64);
        env.storage().instance().set(&INIT, &true);
    }

    /// Deposit `amount` of principal into the strategy on behalf of `user`.
    pub fn deposit(env: Env, user: Address, amount: u64) {
        user.require_auth();
        Self::require_initialized(&env);
        if amount == 0 {
            panic!("amount must be > 0");
        }

        Self::strategy_compound(&env);

        let mut pos = Self::load_position(&env, &user);
        pos.principal = pos.principal.checked_add(amount).expect("overflow");
        Self::save_position(&env, &user, &pos);

        let total: u64 = env.storage().instance().get(&TOTAL_PRINCIPAL).unwrap_or(0u64);
        env.storage()
            .instance()
            .set(&TOTAL_PRINCIPAL, &total.checked_add(amount).expect("overflow"));
    }

    /// Harvest and return the user's accumulated yield.
    pub fn harvest(env: Env, user: Address) -> u64 {
        user.require_auth();
        Self::require_initialized(&env);

        Self::strategy_compound(&env);

        let mut pos = Self::load_position(&env, &user);
        if pos.principal == 0 {
            panic!("nothing to harvest");
        }

        let pending = Self::compute_pending_yield(&env, &pos);
        pos.yield_debt = 0;
        pos.last_update = env.ledger().timestamp();
        Self::save_position(&env, &user, &pos);

        pending
    }

    /// Withdraw `amount` of principal from the strategy.
    pub fn withdraw(env: Env, user: Address, amount: u64) -> u64 {
        user.require_auth();
        Self::require_initialized(&env);
        if amount == 0 {
            panic!("amount must be > 0");
        }

        Self::strategy_compound(&env);

        let mut pos = Self::load_position(&env, &user);
        if amount > pos.principal {
            panic!("insufficient principal");
        }

        let pending = Self::compute_pending_yield(&env, &pos);
        pos.principal -= amount;
        pos.yield_debt = 0;
        pos.last_update = env.ledger().timestamp();
        Self::save_position(&env, &user, &pos);

        let total: u64 = env.storage().instance().get(&TOTAL_PRINCIPAL).unwrap_or(0u64);
        // FIX: wrap result in & so it matches the expected &_ reference type
        env.storage()
            .instance()
            .set(&TOTAL_PRINCIPAL, &(total - amount));

        amount + pending
    }

    /// Manually trigger compounding for the calling user.
    pub fn compound(env: Env, user: Address) -> u64 {
        user.require_auth();
        Self::require_initialized(&env);

        Self::strategy_compound(&env);

        let mut pos = Self::load_position(&env, &user);
        if pos.principal == 0 {
            return 0;
        }
        let pending = Self::compute_pending_yield(&env, &pos);
        pos.principal = pos
            .principal
            .checked_add(pending)
            .expect("overflow");
        pos.yield_debt = 0;
        pos.last_update = env.ledger().timestamp();
        Self::save_position(&env, &user, &pos);

        pending
    }

    /// View the user's currently unharvested yield. Does not change state.
    pub fn pending_yield(env: Env, user: Address) -> u64 {
        Self::require_initialized(&env);
        let pos = Self::load_position(&env, &user);
        if pos.principal == 0 {
            return 0;
        }
        Self::compute_pending_yield(&env, &pos)
    }

    /// View the total principal currently deposited in the strategy.
    pub fn total_principal(env: Env) -> u64 {
        env.storage().instance().get(&TOTAL_PRINCIPAL).unwrap_or(0u64)
    }

    // ---------- admin ----------

    /// Update the yield rate. Only callable by the strategy owner.
    pub fn set_rate(env: Env, new_rate_bps: u64) {
        let owner: Address = env.storage().instance().get(&OWNER).expect("not initialized");
        owner.require_auth();
        if new_rate_bps > 10_000 {
            panic!("rate_bps exceeds 100% per period");
        }
        Self::strategy_compound(&env);
        env.storage().instance().set(&RATE_BPS, &new_rate_bps);
    }

    // ---------- internal helpers ----------

    fn require_initialized(env: &Env) {
        if !env.storage().instance().has(&INIT) {
            panic!("not initialized");
        }
    }

    fn strategy_compound(env: &Env) {
        let now = env.ledger().timestamp();
        let last: u64 = env.storage().instance().get(&LAST_COMPOUND).unwrap_or(now);
        let period: u64 = env.storage().instance().get(&PERIOD).expect("not initialized");
        if now <= last || period == 0 {
            return;
        }
        env.storage().instance().set(&LAST_COMPOUND, &now);
    }

    fn compute_pending_yield(env: &Env, pos: &Position) -> u64 {
        let rate_bps: u64 = env.storage().instance().get(&RATE_BPS).expect("not initialized");
        let period: u64 = env.storage().instance().get(&PERIOD).expect("not initialized");
        let now = env.ledger().timestamp();

        if now <= pos.last_update || period == 0 || pos.principal == 0 {
            return pos.yield_debt;
        }

        let elapsed = now - pos.last_update;
        let periods = elapsed / period;
        if periods == 0 {
            return pos.yield_debt;
        }

        let accrued = pos
            .principal
            .checked_mul(rate_bps)
            .and_then(|v| v.checked_mul(periods))
            .map(|v| v / 10_000)
            .unwrap_or(0);

        pos.yield_debt + accrued
    }

    // FIX: Use DataKey enum instead of raw tuple (Symbol, Address)
    fn load_position(env: &Env, user: &Address) -> Position {
        env.storage()
            .persistent()
            .get(&DataKey::Position(user.clone()))
            .unwrap_or(Position {
                principal: 0,
                yield_debt: 0,
                last_update: env.ledger().timestamp(),
            })
    }

    fn save_position(env: &Env, user: &Address, pos: &Position) {
        env.storage()
            .persistent()
            .set(&DataKey::Position(user.clone()), pos);
    }
}