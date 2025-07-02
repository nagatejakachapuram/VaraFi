#![no_std]
#![allow(static_mut_refs)]

use extended_vft_client::vft::io as vft_io;
use sails_rs::prelude::*;
extern crate alloc;
use alloc::collections::BTreeMap;
use sails_rs::{ service, program };
use sails_rs::prelude::ActorId;
use sails_rs::gstd::msg;
use sails_rs::gstd::exec::block_timestamp;

// Fixed decimal constants
const WAD: u128 = 1_000_000_000_000_000_000; // 18 decimals for calculations
const TVARA_UNIT: u128 = 1_000_000_000_000; // 12 decimals for TVARA/VFT tokens
const DEFAULT_TVARA_PRICE: u128 = WAD; // 1 TVARA = 1 USD (in 18 decimal format for calculations)

static mut STORAGE: Option<LendingStorage> = None;

#[derive(Clone, Debug)]
pub struct LendingStorage {
    pub vft_address: ActorId,
    pub collateral: BTreeMap<ActorId, u128>, // in TVARA units (12 decimals)
    pub tvara_price: u128, // in 18 decimal format for calculations
    pub debt: BTreeMap<ActorId, u128>, // in TVARA units (12 decimals) - simplified!
    pub lender_balances: BTreeMap<ActorId, u128>, // in TVARA units (12 decimals)
    pub lender_interest_earned: BTreeMap<ActorId, u128>,
    pub total_liquidity: u128, // in TVARA units (12 decimals)
    pub treasury: u128, // in TVARA units (12 decimals)
    pub paused: bool,
    pub reentrancy: bool,
    pub admin: ActorId,
    pub last_accrual_ts: u64,
    pub total_interest_earned: u128,
    // Track accrued interest per user in TVARA units
    pub user_accrued_interest: BTreeMap<ActorId, u128>, // in TVARA units (12 decimals)
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
    pub collateral_to_return : u128,
    pub interest_deducted : u128
    pub debt_fully_paid : bool
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

#[derive(Encode, TypeInfo, Clone)]
pub struct UserInfo {
    pub collateral: u128,
    pub debt: u128,
    pub lender_balance: u128,
    pub tvara_price: u128,
    pub health_factor: u128,
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

    pub fn get_mut(&mut self) -> &'static mut LendingStorage {
        unsafe { STORAGE.as_mut().expect("Lending protocol is not initialized") }
    }

    pub fn get(&self) -> &'static LendingStorage {
        unsafe { STORAGE.as_ref().expect("Lending protocol is not initialized") }
    }
}

#[service(events = LendingEvent)]
impl LendingService {
    pub async fn init(vft_address: ActorId) -> Self {
        unsafe {
            STORAGE = Some(LendingStorage {
                vft_address,
                tvara_price: DEFAULT_TVARA_PRICE,
                collateral: BTreeMap::new(),
                debt: BTreeMap::new(),
                lender_balances: BTreeMap::new(),
                lender_interest_earned: BTreeMap::new(),
                total_liquidity: 0,
                treasury: 0,
                paused: false,
                reentrancy: false,
                admin: msg::source(),
                last_accrual_ts: block_timestamp(),
                total_interest_earned: 0,
                user_accrued_interest: BTreeMap::new(),
            });
        }
        Self(())
    }

    fn get_price(storage: &LendingStorage) -> u128 {
        storage.tvara_price
    }

    pub fn update_tvara_price(&mut self, new_price: u128) {
        let storage = self.get_mut();
        assert_eq!(msg::source(), storage.admin, "Only admin can update price");
        assert!(new_price > 0, "Price must be positive");
        storage.tvara_price = new_price;
    }

    pub fn get_user_info(&self, user: ActorId) -> UserInfo {
        let storage = self.get();
        let collateral = *storage.collateral.get(&user).unwrap_or(&0);
        let debt = *storage.debt.get(&user).unwrap_or(&0);
        let lender_balance = *storage.lender_balances.get(&user).unwrap_or(&0);
        let price = storage.tvara_price;

        UserInfo {
            collateral,
            debt,
            lender_balance,
            tvara_price: price,
            health_factor: if debt > 0 {
                self.get_health_factor(user)
            } else {
                u128::MAX
            },
        }
    }

    pub fn get_user_position(&self, user: ActorId) -> (u128, u128, u128, u128) {
        let storage = self.get();
        let collateral = *storage.collateral.get(&user).unwrap_or(&0);
        let debt = *storage.debt.get(&user).unwrap_or(&0);
        let price = storage.tvara_price;
        // Convert TVARA collateral to value using price (for 18-decimal calculations)
        let collateral_value = (collateral * price) / TVARA_UNIT;

        (collateral, debt, collateral_value, price)
    }

    fn utilization_rate(storage: &LendingStorage) -> u128 {
        let borrowed: u128 = storage.debt.values().sum();
        let total = storage.total_liquidity + borrowed;
        if total == 0 {
            0
        } else {
            // Use WAD for percentage calculations
            (borrowed * WAD) / total
        }
    }

    pub fn get_tvara_price(&self) -> u128 {
        self.get().tvara_price
    }

    fn borrow_rate_per_year(storage: &LendingStorage) -> u128 {
        let u = Self::utilization_rate(storage);
        let r0 = (6 * WAD) / 100;
        let rmax = (10 * WAD) / 100;
        let u_opt = (8 * WAD) / 10;

        if u <= u_opt {
            r0 + (u * (rmax - r0)) / u_opt
        } else {
            r0 + (u_opt * (rmax - r0)) / u_opt + ((u - u_opt) * (rmax - r0)) / (WAD - u_opt)
        }
    }

    fn accrue_interest(&mut self) {
        let now = block_timestamp();  // current blockTimestamp 
        let dt = now - self.get().last_accrual_ts;
        if dt == 0 {
            return;
        }
        let storage = self.get_mut();
        storage.last_accrual_ts = now;

        let rate = Self::borrow_rate_per_year(storage);
        let sec_per_year = 365u128 * 24 * 3600;

        for (user, debt) in storage.debt.iter_mut() {
            // Calculate interest in TVARA terms directly
            let interest = (*debt * rate * (dt as u128)) / sec_per_year / WAD;
            // *debt += interest;
            
            // Track accrued interest per user in TVARA units
            *storage.user_accrued_interest.entry(*user).or_default() += interest;
            
            let fee = (interest * 2) / 100; // 2% fee
            storage.treasury += fee;
            storage.total_liquidity += interest - fee;
        }
    }

    fn guard<F, R>(&mut self, f: F) -> R where F: FnOnce(&mut LendingStorage) -> R {
        self.accrue_interest();
        let storage = self.get_mut();
        assert!(!storage.paused, "Protocol is paused");
        assert!(!storage.reentrancy, "Reentrant call");
        storage.reentrancy = true;
        let res = f(storage);
        storage.reentrancy = false;
        res
    }

    pub fn deposit_collateral(&mut self) {
        let amount = msg::value();
        assert!(amount > 0, "Must send VARA tokens as collateral");
        let user = msg::source();

        self.guard(|storage| {
            *storage.collateral.entry(user).or_default() += amount;
        });

        let _ = self.emit_event(
            LendingEvent::CollateralDeposited(CollateralDeposited { user, amount })
        );
    }

    pub async fn borrow(&mut self) {
        let user = msg::source();
        let (vft_address, mint_amount) = self.guard(|storage| {
            let collateral_amount = *storage.collateral.get(&user).unwrap_or(&0);
            assert!(collateral_amount > 0, "No collateral deposited");

            let price = Self::get_price(storage);
            // Convert collateral to value for LTV calculations (18 decimal precision)
            let collateral_value = (collateral_amount * price) / TVARA_UNIT;

            let max_borrowable_value = (collateral_value * 100) / 150; // 150% LTV cap
            let borrow_value = (collateral_value * 66) / 100; // ~66% safe LTV

            // Convert borrow value back to TVARA units for debt tracking
            let borrow_amount = (borrow_value * TVARA_UNIT) / price;

            let current_debt = *storage.debt.get(&user).unwrap_or(&0);
            let max_borrowable = (max_borrowable_value * TVARA_UNIT) / price;
            
            assert!(current_debt + borrow_amount <= max_borrowable, "Exceeds maximum LTV ratio");
            assert!(borrow_amount <= storage.total_liquidity, "Insufficient liquidity");

            // Store debt in TVARA units (12 decimals)
            *storage.debt.entry(user).or_default() += borrow_amount;
            storage.total_liquidity -= borrow_amount;

            (storage.vft_address, borrow_amount)
        });

        let mint_call = vft_io::Mint::encode_call(user, mint_amount.into());
        msg::send_bytes_with_gas_for_reply(vft_address, mint_call, 5_000_000_000, 0, 0)
            .expect("Mint call failed").await
            .expect("Mint failed");

        let _ = self.emit_event(
            LendingEvent::Borrowed(Borrowed {
                user,
                amount: mint_amount,
            })
        );
    }

    // MAIN CHANGE: Updated repay function to deduct interest from collateral
    pub async fn repay(&mut self, user: ActorId, amount: u128) {
        let vft_address = self.get().vft_address;
        let burn_call = vft_io::Burn::encode_call(user, amount.into());
    
        msg::send_bytes_with_gas_for_reply(vft_address, burn_call, 5_000_000_000, 0, 0)
            .expect("Burn call failed").await
            .expect("VFT burn failed - insufficient VFT balance");
    
        let (collateral_to_return, debt_fully_paid, interest_deducted) = self.guard(|storage| {
            let debt_entry = storage.debt.entry(user).or_default();
            let accrued_interest = *storage.user_accrued_interest.get(&user).unwrap_or(&0);
    
            // Only allow repayment up to the principal
            assert!(amount <= *debt_entry, "Cannot repay more than principal debt");
    
            *debt_entry -= amount;
            storage.total_liquidity += amount;
    
            let mut collateral_to_return = 0;
            let mut interest_deducted = 0;
            let debt_fully_paid = *debt_entry == 0;
    
            if debt_fully_paid {
                let collateral_amount = *storage.collateral.get(&user).unwrap_or(&0);
    
                if collateral_amount > accrued_interest {
                    collateral_to_return = collateral_amount - accrued_interest;
                    interest_deducted = accrued_interest;
                    storage.treasury += accrued_interest;
                } else {
                    interest_deducted = collateral_amount;
                    storage.treasury += collateral_amount;
                    collateral_to_return = 0;
                }
    
                storage.collateral.remove(&user);
                storage.debt.remove(&user);
                storage.user_accrued_interest.remove(&user);
            }
    
            (collateral_to_return, debt_fully_paid, interest_deducted)
        });
    
        if debt_fully_paid && collateral_to_return > 0 {
            let sent = msg::send(user, (), collateral_to_return);
            assert!(sent.is_ok(), "Collateral return failed");
        }
    
        let _ = self.emit_event(
            LendingEvent::Repaid(Repaid {
                user,
                amount,
                collateral_to_return,
                interest_deducted, 
                debt_fully_paid
            })
        );
    }
    
    // Additional function for partial collateral withdrawal
    pub fn withdraw_collateral(&mut self, user: ActorId, amount: u128) {
        let collateral_to_return = self.guard(|storage| {
            let collateral_amount = *storage.collateral.get(&user).unwrap_or(&0);
            let debt_amount = *storage.debt.get(&user).unwrap_or(&0);

            assert!(collateral_amount >= amount, "Insufficient collateral");

            let remaining = collateral_amount - amount;
            if debt_amount > 0 {
                let price = Self::get_price(storage);
                let remaining_value = (remaining * price) / TVARA_UNIT;
                let max_debt_value = (remaining_value * 100) / 150;
                let max_debt = (max_debt_value * TVARA_UNIT) / price;
                
                assert!(
                    debt_amount <= max_debt,
                    "Withdrawal would exceed LTV ratio"
                );
            }

            if remaining == 0 {
                storage.collateral.remove(&user);
            } else {
                *storage.collateral.get_mut(&user).unwrap() = remaining;
            }

            amount
        });

        let sent = msg::send(user, (), collateral_to_return);
        assert!(sent.is_ok(), "Collateral withdrawal failed");
    }

    pub async fn lend(&mut self) {
        let lender = msg::source();
        self.accrue_interest();

        let amount = msg::value();
        assert!(amount > 0, "Lend amount must be > 0");

        self.guard(|storage| {
            *storage.lender_balances.entry(lender).or_default() += amount;
            storage.total_liquidity += amount;
        });

        // Mint VFT tokens equivalent to `amount`
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

    pub async fn withdraw(&mut self, amount: u128) {
        let lender = msg::source();
        self.accrue_interest();

        let vft = self.get().vft_address;
        let burn_call = vft_io::Burn::encode_call(lender, amount.into());

        msg::send_bytes_with_gas_for_reply(vft, burn_call, 5_000_000_000, 0, 0)
            .expect("Burn call failed").await
            .expect("Burn failed");

        self.guard(|storage| {
            let bal = storage.lender_balances.entry(lender).or_default();   // lender balance : 10000000000000
            assert!(*bal >= amount, "Insufficient balance");    // lender balance - amount_total_burn(In this basically, as equivalent amount of token is minted , so user send  )
            *bal -= amount;
            storage.total_liquidity -= amount;

            let sent = msg::send(lender, (), amount);
            assert!(sent.is_ok(), "VARA transfer failed");
        });

        let _ = self.emit_event(
            LendingEvent::LiquidityWithdrawn(LiquidityWithdrawn {
                lender,
                amount,
            })
        );
    }

    pub fn liquidate(&mut self, user: ActorId) {
        let (collateral_sold, debt_cleared) = self.guard(|storage| {
            let collateral_amount = *storage.collateral.get(&user).unwrap_or(&0);
            let debt_amount = *storage.debt.get(&user).unwrap_or(&0);

            assert!(collateral_amount > 0, "No collateral to liquidate");
            assert!(debt_amount > 0, "No debt to liquidate");

            let price = Self::get_price(storage);
            let collateral_value = (collateral_amount * price) / TVARA_UNIT;
            let debt_value = (debt_amount * price) / TVARA_UNIT;

            let health = (collateral_value * 100) / debt_value;
            assert!(health < 120, "Position not eligible for liquidation");

            storage.collateral.remove(&user);
            storage.debt.remove(&user);
            storage.user_accrued_interest.remove(&user);
            storage.total_liquidity += collateral_amount;

            (collateral_amount, debt_amount)
        });

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
            return u128::MAX;
        }

        // Health factor = (collateral_value * liquidation_threshold) / debt_value
        // Both are in TVARA units, so we can compare directly
        // Using 120% as liquidation threshold
        (collateral_amount * 120) / debt_amount
    }

    // Helper functions
    pub fn get_user_accrued_interest(&self, user: ActorId) -> u128 {
        *self.get().user_accrued_interest.get(&user).unwrap_or(&0)
    }

    pub fn get_treasury_balance(&self) -> u128 {
        self.get().treasury
    }
}

pub struct BlockchainProgram(());

#[program]
impl BlockchainProgram {
    pub async fn new(vft_address: ActorId) -> Self {
        LendingService::init(vft_address).await;
        Self(())
    }

    pub fn lending_service(&self) -> LendingService {
        LendingService::new()
    }
}