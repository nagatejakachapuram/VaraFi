#![no_std]
#![allow(static_mut_refs)]
#[warn(dead_code)]
use extended_vft_client::vft::io as vft_io;
use sails_rs::prelude::*;
extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use parity_scale_codec::{Decode, Encode};
use sails_rs::gstd::exec::block_timestamp;
use sails_rs::gstd::msg;
use sails_rs::prelude::ActorId;
use sails_rs::{program, service}; // Import energy_balance
use scale_info::TypeInfo;

// Fixed decimal constants
const WAD: u128 = 1_000_000_000_000_000_000; // 18 decimals for calculations
const TVARA_UNIT: u128 = 1_000_000_000_000; // 12 decimals for TVARA/VFT tokens
const DEFAULT_TVARA_PRICE: u128 = WAD; // 1 TVARA = 1 USD (in 18 decimal format for calculations)

// Interest distribution percentages (as fractions of 100)
const LENDER_INTEREST_SHARE: u128 = 4; // 4% for lenders
const TREASURY_INTEREST_SHARE: u128 = 2; // 2% for treasury
#[allow(dead_code)]
const TOTAL_INTEREST_SHARE_PERCENT: u128 = LENDER_INTEREST_SHARE + TREASURY_INTEREST_SHARE; // Total 6% from accrued interest

static mut STORAGE: Option<LendingStorage> = None;
// === State Struct ===
#[derive(State, Debug)]
pub struct LendingContract {
    #[state]
    pub vft_address: Var<ActorId>,
}

// === Init Message ===
#[derive(Encode, Decode, TypeInfo, Clone, Debug)]
pub struct LendingInit {
    pub vft_address: ActorId,
}

// === Contract Impl (Init Handler) ===
impl Contract for LendingContract {
    type Message = LendingInit;
    type Response = ();

    fn handle(&mut self, _ctx: &Context, msg: Self::Message) -> Self::Response {
        self.vft_address.set(msg.vft_address);
    }
}

#[derive(Clone, Debug)]
pub struct LendingStorage {
    pub vft_address: ActorId,
    pub collateral: BTreeMap<ActorId, u128>, // in TVARA units (12 decimals)
    pub tvara_price: u128,                   // in 18 decimal format for calculations
    pub debt: BTreeMap<ActorId, u128>, // in TVARA units (12 decimals) - this will ONLY track PRINCIPAL debt
    pub lender_balances: BTreeMap<ActorId, u128>, // in TVARA units (12 decimals)
    pub lender_interest_earned: BTreeMap<ActorId, u128>, // New: Tracks interest earned by each lender
    pub total_liquidity: u128,                           // in TVARA units (12 decimals)
    pub treasury: u128,                                  // in TVARA units (12 decimals)
    pub paused: bool,
    pub reentrancy: bool,
    pub admin: ActorId,
    pub last_accrual_ts: u64,
    pub total_interest_earned: u128, // Keep this, but its purpose changes slightly (now total interest generated)
    pub user_accrued_interest: BTreeMap<ActorId, u128>, // Tracks accrued interest per borrower
    pub total_principal_borrowed: u128, // New: Sum of all principal debt
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
    pub amount: u128, // This is the principal amount repaid via VFT tokens
    pub collateral_to_return: u128,
    pub interest_deducted: u128, // This is the interest paid from collateral
    pub debt_fully_paid: bool,   // This means principal debt is fully paid
}

#[derive(Encode, TypeInfo, Clone)]
pub struct Liquidated {
    pub user: ActorId,
    pub collateral_sold: u128,
    pub debt_cleared: u128, // This should reflect total debt (principal + accrued interest)
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
pub struct InterestClaimed {
    pub lender: ActorId,
    pub amount: u128,
}

#[derive(Encode, TypeInfo, Clone)]
pub struct UserInfo {
    pub collateral: u128,
    pub debt: u128, // This will be principal debt
    pub lender_balance: u128,
    pub tvara_price: u128,
    pub health_factor: u128,
    pub accrued_interest: u128, // Added for clarity, as interest is separate
    pub lender_interest_earned: u128, // New: Lender's earned interest
}

#[derive(Encode, TypeInfo)]
pub enum LendingEvent {
    CollateralDeposited(CollateralDeposited),
    Borrowed(Borrowed),
    Repaid(Repaid),
    Liquidated(Liquidated),
    LiquidityProvided(LiquidityProvided),
    LiquidityWithdrawn(LiquidityWithdrawn),
    InterestClaimed(InterestClaimed), // New event
}

pub struct LendingService(());

impl LendingService {
    pub fn new() -> Self {
        Self(())
    }

    pub fn get_mut(&mut self) -> &'static mut LendingStorage {
        unsafe {
            STORAGE
                .as_mut()
                .expect("Lending protocol is not initialized")
        }
    }

    pub fn get(&self) -> &'static LendingStorage {
        unsafe {
            STORAGE
                .as_ref()
                .expect("Lending protocol is not initialized")
        }
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
                lender_interest_earned: BTreeMap::new(), // Initialize new map
                total_liquidity: 0,
                treasury: 0,
                paused: false,
                reentrancy: false,
                admin: msg::source(),
                last_accrual_ts: block_timestamp(),
                total_interest_earned: 0,
                user_accrued_interest: BTreeMap::new(),
                total_principal_borrowed: 0, // Initialize new field
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
        let debt = *storage.debt.get(&user).unwrap_or(&0); // Principal debt
        let accrued_interest = *storage.user_accrued_interest.get(&user).unwrap_or(&0);
        let lender_balance = *storage.lender_balances.get(&user).unwrap_or(&0);
        let lender_earned = *storage.lender_interest_earned.get(&user).unwrap_or(&0); // New
        let price = storage.tvara_price;

        UserInfo {
            collateral,
            debt, // Principal debt
            lender_balance,
            tvara_price: price,
            health_factor: if debt > 0 || accrued_interest > 0 {
                // Check if any debt (principal or interest) exists
                self.get_health_factor(user) // This function will now consider total debt
            } else {
                u128::MAX
            },
            accrued_interest, // Explicitly include accrued interest
            lender_interest_earned: lender_earned, // Include lender's earned interest
        }
    }

    pub fn get_user_position(&self, user: ActorId) -> (u128, u128, u128, u128) {
        let storage = self.get();
        let collateral = *storage.collateral.get(&user).unwrap_or(&0);
        let debt = *storage.debt.get(&user).unwrap_or(&0); // This is principal debt
        let price = storage.tvara_price;
        // Convert TVARA collateral to value using price (for 18-decimal calculations)
        let collateral_value = (collateral * price) / TVARA_UNIT;

        (collateral, debt, collateral_value, price)
    }

    // Public view function to get utilization rate
    pub fn get_utilization_rate(&self) -> u128 {
        Self::utilization_rate(self.get())
    }

    fn utilization_rate(storage: &LendingStorage) -> u128 {
        // When calculating utilization, we should consider all borrowed TVARA,
        // which includes principal debt + currently outstanding accrued interest.
        let borrowed_principal: u128 = storage.total_principal_borrowed; // Use the sum
        let borrowed_interest: u128 = storage.user_accrued_interest.values().sum();
        let total_borrowed = borrowed_principal + borrowed_interest;

        let total = storage.total_liquidity + total_borrowed; // Total TVARA in the system (available + borrowed)

        if total == 0 {
            0
        } else {
            // Use WAD for percentage calculations
            (total_borrowed * WAD) / total
        }
    }

    pub fn get_tvara_price(&self) -> u128 {
        self.get().tvara_price
    }

    // Public view function to get borrow rate per year
    pub fn get_borrow_rate_per_year(&self) -> u128 {
        Self::borrow_rate_per_year(self.get())
    }

    fn borrow_rate_per_year(storage: &LendingStorage) -> u128 {
        let u = Self::utilization_rate(storage);
        let r0 = (6 * WAD) / 100; // Base rate is 6% per year (total interest rate)
        let rmax = (10 * WAD) / 100; // Max rate is 10% per year (total interest rate)
        let u_opt = (8 * WAD) / 10; // Optimal utilization at 80%

        if u <= u_opt {
            r0 + (u * (rmax - r0)) / u_opt
        } else {
            // Linear interpolation beyond u_opt
            // r0_prime = r0 + (u_opt * (rmax - r0)) / u_opt (rate at u_opt)
            // r_slope_high = (rmax - r0_prime) / (WAD - u_opt)
            // r_current = r0_prime + (u - u_opt) * r_slope_high
            r0 + (u_opt * (rmax - r0)) / u_opt + ((u - u_opt) * (rmax - r0)) / (WAD - u_opt)
        }
    }

    fn accrue_interest(&mut self) {
        let now = block_timestamp();
        let dt = now - self.get().last_accrual_ts;
        if dt == 0 {
            return;
        }
        let storage = self.get_mut();
        storage.last_accrual_ts = now;

        let rate = Self::borrow_rate_per_year(storage);
        let sec_per_year = 365u128 * 24 * 3600;

        // Iterate over principal debts to accrue interest
        let users_with_debt: Vec<ActorId> = storage.debt.keys().cloned().collect();

        // Calculate total new interest generated in this period
        // This is based on total principal borrowed
        let total_new_interest_generated =
            (storage.total_principal_borrowed * rate * (dt as u128)) / sec_per_year / WAD;

        if total_new_interest_generated > 0 {
            // Distribute interest to treasury and lenders
            let treasury_cut = (total_new_interest_generated * TREASURY_INTEREST_SHARE) / 100;
            storage.treasury += treasury_cut;

            let lender_share_total = (total_new_interest_generated * LENDER_INTEREST_SHARE) / 100;
            storage.total_interest_earned += total_new_interest_generated; // This now tracks total interest generated

            // Distribute lender share proportionally
            if storage.total_liquidity > 0 && lender_share_total > 0 {
                let lenders: Vec<ActorId> = storage.lender_balances.keys().cloned().collect();
                for lender in lenders {
                    if let Some(&balance) = storage.lender_balances.get(&lender) {
                        if balance > 0 {
                            let lender_share =
                                (balance * lender_share_total) / storage.total_liquidity;
                            *storage.lender_interest_earned.entry(lender).or_default() +=
                                lender_share;
                        }
                    }
                }
            }

            // The 'accrued interest' on the borrower's side is the total interest generated
            // that is applied to their debt.
            // This part needs to be adjusted. The `total_new_interest_generated` is the *pool* of interest.
            // We need to apply the specific borrower's interest to their debt.
            // The 6% total interest is applied to the borrower's debt.
            for user in users_with_debt {
                if let Some(&debt_amount) = storage.debt.get(&user) {
                    if debt_amount == 0 {
                        continue;
                    }
                    let borrower_interest =
                        (debt_amount * rate * (dt as u128)) / sec_per_year / WAD;
                    if borrower_interest > 0 {
                        *storage.user_accrued_interest.entry(user).or_default() +=
                            borrower_interest;
                    }
                }
            }
        }
    }

    fn guard<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut LendingStorage) -> R,
    {
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

        let _ = self.emit_event(LendingEvent::CollateralDeposited(CollateralDeposited {
            user,
            amount,
        }));
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

            // When checking against max_borrowable, we need to sum principal debt AND accrued interest
            let current_principal_debt = *storage.debt.get(&user).unwrap_or(&0);
            let current_accrued_interest = *storage.user_accrued_interest.get(&user).unwrap_or(&0);
            let total_current_debt = current_principal_debt + current_accrued_interest;

            let max_borrowable = (max_borrowable_value * TVARA_UNIT) / price;

            assert!(
                total_current_debt + borrow_amount <= max_borrowable,
                "Exceeds maximum LTV ratio"
            );
            assert!(
                borrow_amount <= storage.total_liquidity,
                "Insufficient liquidity"
            );

            // Store new debt as principal
            *storage.debt.entry(user).or_default() += borrow_amount;
            storage.total_principal_borrowed += borrow_amount; // Update total principal borrowed
            storage.total_liquidity -= borrow_amount;

            (storage.vft_address, borrow_amount)
        });

        let mint_call = vft_io::Mint::encode_call(user, mint_amount.into());
        msg::send_bytes_with_gas_for_reply(vft_address, mint_call, 5_000_000_000, 0, 0)
            .expect("Mint call failed")
            .await
            .expect("Mint failed");

        let _ = self.emit_event(LendingEvent::Borrowed(Borrowed {
            user,
            amount: mint_amount,
        }));
    }

    // Repay function: 'amount' repays principal. Interest is deducted from collateral when principal is 0.
    pub async fn repay(&mut self, user: ActorId, amount: u128) {
        let vft_address = self.get().vft_address;
        let burn_call = vft_io::Burn::encode_call(user, amount.into());

        msg::send_bytes_with_gas_for_reply(vft_address, burn_call, 5_000_000_000, 0, 0)
            .expect("Burn call failed")
            .await
            .expect("VFT burn failed - insufficient VFT balance");

        let (collateral_to_return, debt_fully_paid, interest_deducted) = self.guard(|storage| {
            let principal_debt_entry = storage.debt.entry(user).or_default();
            let accrued_interest = *storage.user_accrued_interest.get(&user).unwrap_or(&0);

            let amount_repaid_principal = core::cmp::min(amount, *principal_debt_entry);

            *principal_debt_entry -= amount_repaid_principal;
            storage.total_principal_borrowed -= amount_repaid_principal; // Update total principal borrowed
            storage.total_liquidity += amount_repaid_principal; // Principal repaid returns to liquidity

            let mut collateral_to_return_val = 0;
            let mut interest_deducted_val = 0;
            let principal_debt_is_zero = *principal_debt_entry == 0;

            // Only if principal debt is fully paid, handle interest deduction from collateral
            if principal_debt_is_zero {
                let collateral_amount = *storage.collateral.get(&user).unwrap_or(&0);

                if collateral_amount >= accrued_interest {
                    // Collateral is sufficient to cover all accrued interest
                    collateral_to_return_val = collateral_amount - accrued_interest;
                    interest_deducted_val = accrued_interest;
                    // Note: Treasury and lenders already got their share via accrue_interest.
                    // This is simply the transfer from collateral to cover the accrued interest.
                } else {
                    // Collateral is less than accrued interest, deduct all available collateral
                    interest_deducted_val = collateral_amount;
                    collateral_to_return_val = 0;
                }

                // Clean up user's entries after full principal repayment and interest settlement
                storage.collateral.remove(&user);
                storage.debt.remove(&user); // Principal debt is zero, so remove entry
                storage.user_accrued_interest.remove(&user); // Accrued interest has been settled
            }

            (
                collateral_to_return_val,
                principal_debt_is_zero,
                interest_deducted_val,
            )
        });

        if debt_fully_paid && collateral_to_return > 0 {
            let sent = msg::send(user, (), collateral_to_return);
            assert!(sent.is_ok(), "Collateral return failed");
        }

        let _ = self.emit_event(LendingEvent::Repaid(Repaid {
            user,
            amount, // This is the total VFT amount sent by user
            collateral_to_return,
            interest_deducted,
            debt_fully_paid, // Indicates if principal debt reached zero
        }));
    }

    // Additional function for partial collateral withdrawal
    pub fn withdraw_collateral(&mut self, user: ActorId, amount: u128) {
        let collateral_to_return = self.guard(|storage| {
            let collateral_amount = *storage.collateral.get(&user).unwrap_or(&0);
            let principal_debt_amount = *storage.debt.get(&user).unwrap_or(&0);
            let accrued_interest_amount = *storage.user_accrued_interest.get(&user).unwrap_or(&0);

            assert!(collateral_amount >= amount, "Insufficient collateral");

            let remaining_collateral = collateral_amount - amount;

            // If there's any outstanding debt (principal or interest), check LTV
            if principal_debt_amount > 0 || accrued_interest_amount > 0 {
                let price = Self::get_price(storage);
                let remaining_collateral_value = (remaining_collateral * price) / TVARA_UNIT;
                let total_current_debt_value =
                    ((principal_debt_amount + accrued_interest_amount) * price) / TVARA_UNIT;

                let max_allowed_debt_value = (remaining_collateral_value * 100) / 150;

                assert!(
                    total_current_debt_value <= max_allowed_debt_value,
                    "Withdrawal would exceed LTV ratio"
                );
            }

            if remaining_collateral == 0 {
                storage.collateral.remove(&user);
            } else {
                *storage.collateral.get_mut(&user).unwrap() = remaining_collateral;
            }

            amount
        });

        let sent = msg::send(user, (), collateral_to_return);
        assert!(sent.is_ok(), "Collateral withdrawal failed");
    }

    pub async fn lend(&mut self) {
        let lender = msg::source();
        // accrue_interest is called by guard, no need to call it here explicitly

        let amount = msg::value();
        assert!(amount > 0, "Lend amount must be > 0");

        let (vft, amount_to_mint) = self.guard(|storage| {
            *storage.lender_balances.entry(lender).or_default() += amount;
            storage.total_liquidity += amount;
            (storage.vft_address, amount)
        });

        // Mint VFT tokens equivalent to `amount`
        let mint_call = vft_io::Mint::encode_call(lender, amount_to_mint.into());
        msg::send_bytes_with_gas_for_reply(vft, mint_call, 5_000_000_000, 0, 0)
            .expect("Mint call failed")
            .await
            .expect("Mint failed");

        let _ = self.emit_event(LendingEvent::LiquidityProvided(LiquidityProvided {
            lender,
            amount,
        }));
    }

    pub async fn withdraw(&mut self, amount: u128) {
        let lender = msg::source();
        // accrue_interest is called by guard, no need to call it here explicitly

        let (vft, amount_to_burn, earned_interest_to_withdraw) = self.guard(|storage| {
            let bal = storage.lender_balances.entry(lender).or_default();
            let earned_interest_bal = storage.lender_interest_earned.entry(lender).or_default();

            assert!(*bal >= amount, "Insufficient principal balance to withdraw");
            assert!(
                storage.total_liquidity >= amount,
                "Insufficient total liquidity for principal withdrawal"
            );

            *bal -= amount;
            storage.total_liquidity -= amount;

            let earned_to_return = *earned_interest_bal;
            *earned_interest_bal = 0; // Clear earned interest after withdrawal

            (storage.vft_address, amount, earned_to_return)
        });

        // Burn VFT tokens for the principal amount being withdrawn
        let burn_call = vft_io::Burn::encode_call(lender, amount_to_burn.into());
        msg::send_bytes_with_gas_for_reply(vft, burn_call, 5_000_000_000, 0, 0)
            .expect("Burn call failed")
            .await
            .expect("Burn failed");

        let total_amount_to_send = amount_to_burn + earned_interest_to_withdraw;

        let sent = msg::send(lender, (), total_amount_to_send);
        assert!(sent.is_ok(), "VARA transfer failed");

        let _ = self.emit_event(LendingEvent::LiquidityWithdrawn(LiquidityWithdrawn {
            lender,
            amount: amount_to_burn, // This event still refers to principal withdrawn
        }));

        // Optionally, emit a separate event for interest withdrawal if desired
        if earned_interest_to_withdraw > 0 {
            let _ = self.emit_event(LendingEvent::InterestClaimed(InterestClaimed {
                lender,
                amount: earned_interest_to_withdraw,
            }));
        }
    }

    // New function for lenders to claim earned interest separately
    pub fn claim_interest(&mut self) {
        let lender = msg::source();
        let earned_interest_to_claim = self.guard(|storage| {
            let earned_interest_bal = storage.lender_interest_earned.entry(lender).or_default();
            let amount = *earned_interest_bal;
            assert!(amount > 0, "No interest to claim");
            *earned_interest_bal = 0; // Reset balance after claiming
            amount
        });

        let sent = msg::send(lender, (), earned_interest_to_claim);
        assert!(sent.is_ok(), "Interest transfer failed");

        let _ = self.emit_event(LendingEvent::InterestClaimed(InterestClaimed {
            lender,
            amount: earned_interest_to_claim,
        }));
    }

    pub fn liquidate(&mut self, user: ActorId) {
        let (collateral_cleared_amount, debt_cleared_amount) = self.guard(|storage| {
            // 1. Get raw amounts from storage
            let collateral_amount_vara = *storage.collateral.get(&user).unwrap_or(&0); // Collateral is VARA (12 decimals)
            let principal_debt_amount_tvara = *storage.debt.get(&user).unwrap_or(&0); // Principal debt is TVARA (12 decimals)
            let accrued_interest_amount_tvara =
                *storage.user_accrued_interest.get(&user).unwrap_or(&0); // Accrued interest is TVARA (12 decimals)

            // 2. Calculate total TVARA debt first
            let total_debt_tvara = principal_debt_amount_tvara + accrued_interest_amount_tvara;

            // 3. Assertions for valid state (before price calculations)
            assert!(collateral_amount_vara > 0, "No collateral to liquidate");
            assert!(total_debt_tvara > 0, "No debt to liquidate");

            // 4. Get VARA's price from storage (assuming `storage.tvara_price` is now VARA's price in WAD)
            let vara_price_in_wad = storage.tvara_price; // Access directly from storage within the guard

            // 5. Convert amounts to a common 18-decimal USD value for health factor calculation
            //    a. Collateral value in USD: (VARA amount * VARA price in WAD) / TVARA_UNIT (scales 12-decimal VARA to 18-decimal USD)
            let collateral_value_usd = (collateral_amount_vara * vara_price_in_wad) / TVARA_UNIT;

            //    b. Total debt value in USD: (TVARA amount * 1 USD/TVARA fixed price) / TVARA_UNIT (scales 12-decimal TVARA to 18-decimal USD)
            //       Assuming 1 TVARA is fixed at 1 USD (or 1.0 * WAD).
            let total_debt_value_usd = total_debt_tvara * (WAD / TVARA_UNIT);

            // 6. Defensive check to prevent division by zero for health factor
            assert!(
                total_debt_value_usd > 0,
                "Calculated total debt value is zero. Cannot liquidate if debt is zero."
            );

            // 7. Calculate Health Factor based on USD values
            let health = (collateral_value_usd * 100) / total_debt_value_usd;

            // 8. Assert liquidation condition
            assert!(
                health < 120,
                "Position not eligible for liquidation: Health factor is >= 120"
            );

            // --- All checks passed, perform the actual state changes for liquidation ---
            storage.collateral.remove(&user);
            storage.debt.remove(&user);
            storage.user_accrued_interest.remove(&user);
            storage.total_principal_borrowed -= principal_debt_amount_tvara; // Update total principal borrowed
            storage.total_liquidity += collateral_amount_vara; // Return VARA collateral to total liquidity

            // 9. Return the raw amounts that need to be used in the event emission outside the closure
            (collateral_amount_vara, total_debt_tvara) // Return VARA cleared and total TVARA debt cleared
        });

        // 10. Emit the event using the values returned by the closure
        let _ = self.emit_event(LendingEvent::Liquidated(Liquidated {
            user,
            collateral_sold: collateral_cleared_amount, // The amount of VARA collateral that was cleared
            debt_cleared: debt_cleared_amount, // The total TVARA debt (principal + accrued interest) that was cleared
        }));
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
        // This function now explicitly returns ONLY the principal debt
        *self.get().debt.get(&user).unwrap_or(&0)
    }

    // New function to get the total outstanding debt (principal + accrued interest)
    pub fn get_total_outstanding_debt(&self, user: ActorId) -> u128 {
        let storage = self.get();
        let principal_debt = *storage.debt.get(&user).unwrap_or(&0);
        let accrued_interest = *storage.user_accrued_interest.get(&user).unwrap_or(&0);
        principal_debt + accrued_interest
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
        let storage = self.get();
        let collateral_amount_vara = *storage.collateral.get(&user).unwrap_or(&0); // Collateral is VARA (12 decimals)
        let principal_debt_amount_tvara = *storage.debt.get(&user).unwrap_or(&0); // Debt is TVARA (12 decimals)
        let accrued_interest_amount_tvara = *storage.user_accrued_interest.get(&user).unwrap_or(&0); // Accrued interest is TVARA (12 decimals)

        let total_debt_tvara = principal_debt_amount_tvara + accrued_interest_amount_tvara;

        if total_debt_tvara == 0 {
            return u128::MAX; // Loan is perfectly healthy if no debt
        }

        let vara_price_in_wad = storage.tvara_price; // *** THIS IS NOW VARA's PRICE IN WAD (e.g., USD) ***

        // Calculate the USD value of collateral:
        // (VARA amount in 12 decimals * VARA price in 18 decimals) / TVARA_UNIT (to scale down to 18 decimals)
        let collateral_value_usd = (collateral_amount_vara * vara_price_in_wad) / TVARA_UNIT;

        // Calculate the USD value of total TVARA debt:
        // Assuming 1 TVARA = 1 USD (or 1.0 * WAD) for its fixed price
        let total_debt_value_usd = total_debt_tvara * (WAD / TVARA_UNIT); // e.g. 6.6 * 10^12 * 10^6 = 6.6 * 10^18

        if total_debt_value_usd == 0 {
            return u128::MAX;
        }

        // Health factor = (Collateral Value in USD * 100) / (Total Debt Value in USD)
        (collateral_value_usd * 100) / total_debt_value_usd
    }

    // Helper function for user's currently accrued interest
    pub fn get_user_accrued_interest(&self, user: ActorId) -> u128 {
        *self.get().user_accrued_interest.get(&user).unwrap_or(&0)
    }

    // New view function for lender's earned interest
    pub fn get_lender_earned_interest(&self, user: ActorId) -> u128 {
        *self.get().lender_interest_earned.get(&user).unwrap_or(&0)
    }

    pub fn get_treasury_balance(&self) -> u128 {
        self.get().treasury
    }

    pub fn get_last_accrual_ts(&self) -> u64 {
        self.get().last_accrual_ts
    }

    pub fn get_total_principal_borrowed(&self) -> u128 {
        self.get().total_principal_borrowed
    }

    pub fn get_lender_interest_earned(&self, lender: ActorId) -> u128 {
        *self.get().lender_interest_earned.get(&lender).unwrap_or(&0)
    }

    // --- New Function 1: Get all borrowers and their full info ---
    pub fn get_all_borrowers_info(&self) -> BTreeMap<ActorId, UserInfo> {
        let storage = self.get();
        let mut borrowers_info: BTreeMap<ActorId, UserInfo> = BTreeMap::new();

        // We only care about users who actually have debt, so iterate over the debt map
        for borrower_id in storage.debt.keys() {
            let user_info = self.get_user_info(*borrower_id);
            borrowers_info.insert(*borrower_id, user_info);
        }
        borrowers_info
    }

    // --- Modified Function: Admin withdraw funds (from total_liquidity) ---
    pub fn admin_withdraw_funds(&mut self, amount_tvara: u128) {
        let recipient = msg::source();
        self.guard(|storage| {
            assert_eq!(
                msg::source(),
                storage.admin,
                "Only admin can withdraw funds"
            );
            assert!(
                amount_tvara > 0,
                "Withdrawal amount must be greater than zero"
            );
            assert!(
                storage.total_liquidity >= amount_tvara,
                "Insufficient total liquidity for withdrawal"
            );

            // Convert the TVARA amount to VARA using the current TVARA price
            // (amount_tvara * price_in_wad) / WAD -- this converts 12-decimal TVARA to 18-decimal VARA value
            // then we convert 18-decimal VARA value to 12-decimal VARA amount (since VARA_UNIT is 12 decimals)
            let vara_to_send = (amount_tvara * storage.tvara_price) / WAD; // Result is in VARA (12 decimals, matching VARA_UNIT)

            storage.total_liquidity -= amount_tvara;

            let sent = msg::send(recipient, (), vara_to_send);
            assert!(
                sent.is_ok(),
                "Admin withdrawal failed or insufficient balance"
            );
        });
    }

    // --- New Function: Admin withdraw treasury funds ---
    pub fn admin_withdraw_treasury(&mut self, amount_tvara: u128) {
        let recipient = msg::source();
        self.guard(|storage| {
            assert_eq!(
                msg::source(),
                storage.admin,
                "Only admin can withdraw treasury funds"
            );
            assert!(
                amount_tvara > 0,
                "Withdrawal amount must be greater than zero"
            );
            assert!(
                storage.treasury >= amount_tvara,
                "Insufficient treasury balance for withdrawal"
            );

            // Convert the TVARA amount to VARA using the current TVARA price
            let vara_to_send = (amount_tvara * storage.tvara_price) / WAD; // Result is in VARA (12 decimals)

            storage.treasury -= amount_tvara;

            let sent = msg::send(recipient, (), vara_to_send);
            assert!(
                sent.is_ok(),
                "Admin treasury withdrawal failed or insufficient balance"
            );
        });
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

pub mod io;
