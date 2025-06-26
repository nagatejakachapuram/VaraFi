## The **blockchain** program

The program workspace includes the following packages:
- `blockchain` is the package allowing to build WASM binary for the program and IDL file for it.  
  The package also includes integration tests for the program in the `tests` sub-folder
- `blockchain-app` is the package containing business logic for the program represented by the `BlockchainService` structure.  
- `blockchain-client` is the package containing the client for the program allowing to interact with it from another program, tests, or
  off-chain client.

// #![no_std]
// use sails_rs::prelude::*;
// extern crate alloc;
// use alloc::collections::BTreeMap;
// use sails_rs::{service, program};
// use sails_rs::prelude::ActorId;
// pub mod vft_service; // Rename the module to snake_case
// use crate::vft_service::VftService; // Now import the actual type



// #[derive(Encode, TypeInfo, Clone)]
// pub struct CollateralDeposited { pub user: ActorId, pub amount: u128 }
// #[derive(Encode, TypeInfo, Clone)]
// pub struct Borrowed { pub user: ActorId, pub amount: u128 }
// #[derive(Encode, TypeInfo, Clone)]
// pub struct Repaid { pub user: ActorId, pub amount: u128 }
// #[derive(Encode, TypeInfo, Clone)]
// pub struct Liquidated { pub user: ActorId, pub collateral_sold: u128, pub debt_cleared: u128 }

// #[derive(Encode, TypeInfo)]
// pub enum LendingEvent {
//     CollateralDeposited(CollateralDeposited),
//     Borrowed(Borrowed),
//     Repaid(Repaid),
//     Liquidated(Liquidated),
// }

// pub struct LendingService {
//     vft: VftService,
//     collateral: BTreeMap<ActorId, u128>,
//     debt: BTreeMap<ActorId, u128>,
//     lender_balances: BTreeMap<ActorId, u128>,
//     total_liquidity: u128,
//     paused: bool,
//     reentrancy: bool,
// }

// #[service(events = LendingEvent)]
// impl LendingService {
//     pub fn new() -> Self {
//         Self {
//             vft: VftService::new(),
//             collateral: BTreeMap::new(),
//             debt: BTreeMap::new(),
//             lender_balances: BTreeMap::new(),
//             total_liquidity: 0,
//             paused: false,
//             reentrancy: false,
//         }
//     }
    

//     fn guard<F, R>(&mut self, f: F) -> R
//     where F: FnOnce(&mut Self) -> R {
//         assert!(!self.paused, "Protocol is paused");
//         assert!(!self.reentrancy, "Reentrant call");
//         self.reentrancy = true;
//         let res = f(self);
//         self.reentrancy = false;
//         res
//     }

//     pub fn deposit_collateral(&mut self, user: ActorId, amount: u128) {
//         self.guard(|s| {
//             assert!(amount > 0, "Deposit > 0");
//             *s.collateral.entry(user).or_default() += amount;
//            let _ =  s.emit_event(LendingEvent::CollateralDeposited(CollateralDeposited { user, amount }));
//         });
//     }

//      pub fn borrow(&mut self, user: ActorId, amount: u128) -> CommandReply<()> {
//         self.guard(|s| {
//             assert!(amount > 0, "Borrow > 0");
//             let col = *s.collateral.get(&user).unwrap_or(&0);
//             let cur = *s.debt.get(&user).unwrap_or(&0);
//             let max = col * 100 / 150;
//             assert!(cur + amount <= max, "Exceeds 150% LTV");
//             assert!(amount <= s.total_liquidity, "Not enough liquidity");
//             *s.debt.entry(user).or_default() += amount;
//             s.total_liquidity -= amount;

//             // return native tokens to borrower
//             let mut reply = CommandReply::new(());
//             reply = reply.with_value(amount);
//             let _ = s.emit_event(LendingEvent::Borrowed(Borrowed { user, amount }));
//             reply
//         })
//     }

//     pub fn repay(&mut self, user: ActorId, amount: u128) -> CommandReply<()> {
//         self.guard(|s| {
//             let debt = s.debt.entry(user).or_default();
//             assert!(*debt > 0, "No debt");
//             let paid = amount.min(*debt);
//             *debt -= paid;
//             s.total_liquidity += paid;
//             let mut reply = CommandReply::new(());
//             reply = reply.with_value(paid);
//             let _ = s.emit_event(LendingEvent::Repaid(Repaid { user, amount: paid }));
//             reply
//         })
//     }

//     pub fn lend(&mut self, lender: ActorId, amount: u128) {
//         self.guard(|s| {
//             assert!(amount > 0, "Lend > 0");
//             *s.lender_balances.entry(lender).or_default() += amount;
//             s.total_liquidity += amount;
//             s.vft.mint(lender, amount);
//         });
//     }

//     pub fn withdraw(&mut self, lender: ActorId, amount: u128) -> CommandReply<()> {
//         self.guard(|s| {
//             let bal = s.lender_balances.entry(lender).or_default();
//             assert!(*bal >= amount, "Insufficient lender balance");
//             *bal -= amount;
//             s.total_liquidity -= amount;
//             s.vft.burn(lender, amount);
//             let mut reply = CommandReply::new(());
//             reply = reply.with_value(amount);
//             reply
//         })
//     }


//     pub fn liquidate(&mut self, user: ActorId) {
//         self.guard(|s| {
//             let col = *s.collateral.get(&user).unwrap_or(&0);
//             let debt = *s.debt.get(&user).unwrap_or(&0);
//             assert!(col * 100 / debt < 120, "Not eligible");
//             s.collateral.remove(&user);
//             s.debt.remove(&user);
//             s.total_liquidity += col;
//             let _ = s.emit_event(LendingEvent::Liquidated(Liquidated { user, collateral_sold: col, debt_cleared: debt }));
//         });
//     }

//     pub fn pause(&mut self) { self.paused = true; }

//     pub fn resume(&mut self) { self.paused = false; }

//     pub fn get_collateral(&self, user: ActorId) -> u128 { *self.collateral.get(&user).unwrap_or(&0) }
//     pub fn get_debt(&self, user: ActorId) -> u128 { *self.debt.get(&user).unwrap_or(&0) }
//     pub fn get_liquidity(&self) -> u128 { self.total_liquidity }
//     pub fn get_lender_balance(&self, user: ActorId) -> u128 { *self.lender_balances.get(&user).unwrap_or(&0) }
// }
// pub struct BlockchainProgram;

// #[program]
// impl BlockchainProgram {
//     pub fn new() -> Self {
//         Self
//     }
//     pub fn lending_service(&self) -> LendingService {
//         LendingService::new()
//     }
//     pub fn vft_service(&self) -> VftService {
//         VftService::new()
//     }
// }
