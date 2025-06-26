#![no_std]
#![allow(static_mut_refs)]

use extended_vft_client::vft::io as vft_io;
use sails_rs::prelude::*;
extern crate alloc;
use alloc::collections::BTreeMap;
use sails_rs::{ service, program };
use sails_rs::prelude::ActorId;
use sails_rs::gstd::msg;

// Global static storage for state persistence
static mut STORAGE: Option<LendingStorage> = None;

#[derive(Clone, Debug)]
pub struct LendingStorage {
    pub vft_address: ActorId,
    pub collateral: BTreeMap<ActorId, u128>,
    pub debt: BTreeMap<ActorId, u128>,
    pub lender_balances: BTreeMap<ActorId, u128>,
    pub total_liquidity: u128,
    pub paused: bool,
    pub reentrancy: bool,
    pub admin: ActorId,
}

#[derive(Encode, TypeInfo, Clone)]
pub struct CollateralDeposited {
    pub user: ActorId,
    pub amount: u128,
}

#[derive(Encode, TypeInfo, Clone)]
pub struct Borrowed {
    pub user: ActorId,
    pub amount: u128,
}

#[derive(Encode, TypeInfo, Clone)]
pub struct Repaid {
    pub user: ActorId,
    pub amount: u128,
}

#[derive(Encode, TypeInfo, Clone)]
pub struct Liquidated {
    pub user: ActorId,
    pub collateral_sold: u128,
    pub debt_cleared: u128,
}

#[derive(Encode, TypeInfo, Clone)]
pub struct LiquidityProvided {
    pub lender: ActorId,
    pub amount: u128,
}

#[derive(Encode, TypeInfo, Clone)]
pub struct LiquidityWithdrawn {
    pub lender: ActorId,
    pub amount: u128,
}

#[derive(Encode, TypeInfo)]
pub enum LendingEvent {
    CollateralDeposited(CollateralDeposited),
    Borrowed(Borrowed),
    Repaid(Repaid),
    Liquidated(Liquidated),
    LiquidityProvided(LiquidityProvided),
    LiquidityWithdrawn(LiquidityWithdrawn),
}

pub struct LendingService(());

impl LendingService {
    pub fn new() -> Self {
        Self(())
    }

    // Get mutable reference to storage
    pub fn get_mut(&mut self) -> &'static mut LendingStorage {
        unsafe { STORAGE.as_mut().expect("Lending protocol is not initialized") }
    }

    // Get immutable reference to storage
    pub fn get(&self) -> &'static LendingStorage {
        unsafe { STORAGE.as_ref().expect("Lending protocol is not initialized") }
    }
}

#[service(events = LendingEvent)]
impl LendingService {
    // Initialize the service
    pub async fn init(vft_address: ActorId) -> Self {
        unsafe {
            STORAGE = Some(LendingStorage {
                vft_address,
                collateral: BTreeMap::new(),
                debt: BTreeMap::new(),
                lender_balances: BTreeMap::new(),
                total_liquidity: 0,
                paused: false,
                reentrancy: false,
                admin: msg::source(),
            });
        }
        Self(())
    }

    fn guard<F, R>(&mut self, f: F) -> R where F: FnOnce(&mut LendingStorage) -> R {
        let storage = self.get_mut();
        assert!(!storage.paused, "Protocol is paused");
        assert!(!storage.reentrancy, "Reentrant call");
        storage.reentrancy = true;
        let res = f(storage);
        storage.reentrancy = false;
        res
    }

    pub fn deposit_collateral(&mut self, user: ActorId, amount: u128) {
        self.guard(|storage| {
            assert!(amount > 0, "Deposit must be > 0");
            *storage.collateral.entry(user).or_default() += amount;
        });

        // Emit event after guard to avoid borrowing conflicts
        let _ = self.emit_event(
            LendingEvent::CollateralDeposited(CollateralDeposited {
                user,
                amount,
            })
        );
    }

    pub fn borrow(&mut self, user: ActorId, amount: u128) -> CommandReply<()> {
        let reply = self.guard(|storage| {
            assert!(amount > 0, "Borrow amount must be > 0");

            let collateral_amount = *storage.collateral.get(&user).unwrap_or(&0);
            let current_debt = *storage.debt.get(&user).unwrap_or(&0);

            // Calculate maximum borrowable amount (LTV = 66.67%, so collateral * 100 / 150)
            let max_borrowable = (collateral_amount * 100) / 150;
            assert!(current_debt + amount <= max_borrowable, "Exceeds maximum LTV ratio");
            assert!(amount <= storage.total_liquidity, "Insufficient liquidity in pool");

            *storage.debt.entry(user).or_default() += amount;
            storage.total_liquidity -= amount;

            // Return native tokens to borrower
            let mut reply = CommandReply::new(());
            reply = reply.with_value(amount);
            reply
        });

        // Emit event after guard to avoid borrowing conflicts
        let _ = self.emit_event(
            LendingEvent::Borrowed(Borrowed {
                user,
                amount,
            })
        );

        reply
    }

    pub fn repay(&mut self, user: ActorId, amount: u128) -> CommandReply<()> {
        let (reply, actual_amount) = self.guard(|storage| {
            let debt_entry = storage.debt.entry(user).or_default();
            assert!(*debt_entry > 0, "No outstanding debt");

            let actual_repay_amount = amount.min(*debt_entry);
            *debt_entry -= actual_repay_amount;
            storage.total_liquidity += actual_repay_amount;

            let mut reply = CommandReply::new(());
            reply = reply.with_value(actual_repay_amount);
            (reply, actual_repay_amount)
        });

        // Emit event after guard to avoid borrowing conflicts
        let _ = self.emit_event(
            LendingEvent::Repaid(Repaid {
                user,
                amount: actual_amount,
            })
        );

        reply
    }

    pub async fn lend(&mut self, lender: ActorId, amount: u128) {
        self.guard(|storage| {
            assert!(amount > 0, "Lend amount must be > 0");
            *storage.lender_balances.entry(lender).or_default() += amount;
            storage.total_liquidity += amount;
        });

        let vft = self.get().vft_address;
        let mint_call = vft_io::Mint::encode_call(lender, amount.into());
        msg::send_bytes_with_gas_for_reply(vft, mint_call, 5_000_000_000, 0, 0)
            .expect("Mint call failed").await
            .expect("Mint failed");

        let _ = self.emit_event(
            LendingEvent::LiquidityProvided(LiquidityProvided {
                lender,
                amount,
            })
        );
    }

    pub async fn withdraw(&mut self, lender: ActorId, amount: u128) -> CommandReply<()> {
        let vft = self.get().vft_address;
        let burn_call = vft_io::Burn::encode_call(lender, amount.into());
        msg::send_bytes_with_gas_for_reply(vft, burn_call, 5_000_000_000, 0, 0)
            .expect("Burn call failed").await
            .expect("Burn failed");

        let reply = self.guard(|storage| {
            let bal = storage.lender_balances.entry(lender).or_default();
            assert!(*bal >= amount, "Insufficient balance");
            *bal -= amount;
            storage.total_liquidity -= amount;

            CommandReply::new(()).with_value(amount)
        });

        let _ = self.emit_event(
            LendingEvent::LiquidityWithdrawn(LiquidityWithdrawn {
                lender,
                amount,
            })
        );
        reply
    }

    pub fn liquidate(&mut self, user: ActorId) {
        let (collateral_sold, debt_cleared) = self.guard(|storage| {
            let collateral_amount = *storage.collateral.get(&user).unwrap_or(&0);
            let debt_amount = *storage.debt.get(&user).unwrap_or(&0);

            assert!(collateral_amount > 0, "No collateral to liquidate");
            assert!(debt_amount > 0, "No debt to liquidate");

            // Check if position is undercollateralized (LTV > 120%)
            assert!(
                (collateral_amount * 100) / debt_amount < 120,
                "Position is not eligible for liquidation"
            );

            // Remove the position
            storage.collateral.remove(&user);
            storage.debt.remove(&user);
            storage.total_liquidity += collateral_amount;

            (collateral_amount, debt_amount)
        });

        // Emit event after guard to avoid borrowing conflicts
        let _ = self.emit_event(
            LendingEvent::Liquidated(Liquidated {
                user,
                collateral_sold,
                debt_cleared,
            })
        );
    }

    // Admin functions
    pub fn pause(&mut self) {
        let storage = self.get_mut();
        assert_eq!(msg::source(), storage.admin, "Only admin can pause");
        storage.paused = true;
    }

    pub fn resume(&mut self) {
        let storage = self.get_mut();
        assert_eq!(msg::source(), storage.admin, "Only admin can resume");
        storage.paused = false;
    }

    // View functions
    pub fn get_collateral(&self, user: ActorId) -> u128 {
        *self.get().collateral.get(&user).unwrap_or(&0)
    }

    pub fn get_debt(&self, user: ActorId) -> u128 {
        *self.get().debt.get(&user).unwrap_or(&0)
    }

    pub fn get_liquidity(&self) -> u128 {
        self.get().total_liquidity
    }

    pub fn get_lender_balance(&self, user: ActorId) -> u128 {
        *self.get().lender_balances.get(&user).unwrap_or(&0)
    }

    pub fn is_paused(&self) -> bool {
        self.get().paused
    }

    pub fn get_admin(&self) -> ActorId {
        self.get().admin
    }

    pub fn get_health_factor(&self, user: ActorId) -> u128 {
        let collateral_amount = self.get_collateral(user);
        let debt_amount = self.get_debt(user);

        if debt_amount == 0 {
            return u128::MAX; // No debt means perfect health
        }

        // Health factor = (collateral_value * liquidation_threshold) / debt_value
        // Using 120% as liquidation threshold
        (collateral_amount * 120) / debt_amount
    }
}

pub struct BlockchainProgram(());

#[program]
impl BlockchainProgram {
    // Program constructor - this initializes the lending service
    pub async fn new(vft_address: ActorId) -> Self {
        LendingService::init(vft_address).await;
        Self(())
    }

    // Return the lending service instance (not creating new each time)
    pub fn lending_service(&self) -> LendingService {
        LendingService::new()
    }
}
