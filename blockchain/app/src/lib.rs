#![no_std]
use sails_rs::prelude::*;
extern crate alloc;
// use core::result::Result;
// #[allow(unused_imports)]
// use core::prelude::v1::*;
use alloc::collections::BTreeMap;
use sails_rs::prelude::*;
use sails_rs::{service, program};
use sails_rs::prelude::ActorId;
// use sails_rs::types::AccountId;

use core::marker::Sized;
// Event data structs
#[derive(Encode, TypeInfo, Clone)]
pub struct CollateralDeposited { pub user: ActorId, pub amount: u128 }
#[derive(Encode, TypeInfo, Clone)]
pub struct Borrowed { pub user: ActorId, pub amount: u128 }
#[derive(Encode, TypeInfo, Clone)]
pub struct Repaid { pub user: ActorId, pub amount: u128 }
#[derive(Encode, TypeInfo, Clone)]
pub struct Liquidated { pub user: ActorId, pub collateral_sold: u128, pub debt_cleared: u128 }

// Event enum
#[derive(Encode, TypeInfo)]
pub enum LendingEvent {
    CollateralDeposited(CollateralDeposited),
    Borrowed(Borrowed),
    Repaid(Repaid),
    Liquidated(Liquidated),
}

// Lending service with CRUD storage + guards
pub struct LendingService {
    collateral: BTreeMap<ActorId, u128>,
    debt: BTreeMap<ActorId, u128>,
    lender_balances: BTreeMap<ActorId, u128>,
    total_liquidity: u128,
    paused: bool,
    reentrancy: bool,
}

#[service(events = LendingEvent)]
impl LendingService {
    pub fn new() -> Self {
        Self {
            collateral: BTreeMap::new(),
            debt: BTreeMap::new(),
            lender_balances: BTreeMap::new(),
            total_liquidity: 0,
            paused: false,
            reentrancy: false,
        }
    }

    // Internal guard for pause and reentrancy handling
    fn guard<F, R>(&mut self, f: F) -> R
    where F: FnOnce(&mut Self) -> R {
        assert!(!self.paused, "Protocol is paused");
        assert!(!self.reentrancy, "Reentrant call");
        self.reentrancy = true;
        let res = f(self);
        self.reentrancy = false;
        res
    }

    // #[command]
    pub fn deposit_collateral(&mut self, user: ActorId, amount: u128) {
        self.guard(|s| {
            assert!(amount > 0, "Deposit > 0");
            *s.collateral.entry(user).or_default() += amount;
            s.emit_event(LendingEvent::CollateralDeposited(CollateralDeposited { user, amount }));
        });
    }

    // #[command]
    pub fn borrow(&mut self, user: ActorId, amount: u128) {
        self.guard(|s| {
            assert!(amount > 0, "Borrow > 0");
            let col = *s.collateral.get(&user).unwrap_or(&0);
            let cur = *s.debt.get(&user).unwrap_or(&0);
            let max = col * 100 / 150;
            assert!(cur + amount <= max, "Exceeds 150% LTV");
            assert!(amount <= s.total_liquidity, "Not enough liquidity");
            *s.debt.entry(user).or_default() += amount;
            s.total_liquidity -= amount;
            s.emit_event(LendingEvent::Borrowed(Borrowed { user, amount }));
        });
    }

    // #[command]
    pub fn repay(&mut self, user: ActorId, amount: u128) {
        self.guard(|s| {
            let debt = s.debt.entry(user).or_default();
            assert!(*debt > 0, "No debt");
            let paid = amount.min(*debt);
            *debt -= paid;
            s.total_liquidity += paid;
            s.emit_event(LendingEvent::Repaid(Repaid { user, amount: paid }));
        });
    }

    // #[command]
    pub fn lend(&mut self, lender: ActorId, amount: u128) {
        self.guard(|s| {
            assert!(amount > 0, "Lend > 0");
            *s.lender_balances.entry(lender).or_default() += amount;
            s.total_liquidity += amount;
        });
    }

    // #[command]
    pub fn withdraw(&mut self, lender: ActorId, amount: u128) {
        self.guard(|s| {
            let bal = s.lender_balances.entry(lender).or_default();
            assert!(*bal >= amount, "Insufficient lender balance");
            *bal -= amount;
            s.total_liquidity -= amount;
        });
    }

    // #[command]
    pub fn liquidate(&mut self, user: ActorId) {
        self.guard(|s| {
            let col = *s.collateral.get(&user).unwrap_or(&0);
            let debt = *s.debt.get(&user).unwrap_or(&0);
            assert!(col * 100 / debt < 120, "Not eligible");
            s.collateral.remove(&user);
            s.debt.remove(&user);
            s.total_liquidity += col;
            s.emit_event(LendingEvent::Liquidated(Liquidated { user, collateral_sold: col, debt_cleared: debt }));
        });
    }

    // #[command]
    pub fn pause(&mut self) { self.paused = true; }

    // #[command]
    pub fn resume(&mut self) { self.paused = false; }

    // Queries
    // #[query] 
    pub fn get_collateral(&self, user: ActorId) -> u128 { *self.collateral.get(&user).unwrap_or(&0) }
    // #[query] 
    pub fn get_debt(&self, user: ActorId) -> u128 { *self.debt.get(&user).unwrap_or(&0) }
    // #[query] 
    pub fn get_liquidity(&self) -> u128 { self.total_liquidity }
    // #[query] 
    pub fn get_lender_balance(&self, user: ActorId) -> u128 { *self.lender_balances.get(&user).unwrap_or(&0) }
}

// Entry point program: constructs service
pub struct BlockchainProgram;

#[program]
impl BlockchainProgram {
    pub fn new() -> LendingService {
        LendingService::new()
    }
}
