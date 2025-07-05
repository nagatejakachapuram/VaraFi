use alloc::string::String;
use parity_scale_codec::{Decode, Encode};
use sails_rs::prelude::ActorId;
use scale_info::TypeInfo;

#[derive(Encode, Decode, TypeInfo, Clone, Debug)]
pub struct LendingInit {
    pub vft_address: ActorId,
}

#[derive(Encode, Decode, TypeInfo, Clone, Debug)]
pub enum LendingAction {
    DepositCollateral,
    Borrow,
    Repay { user: ActorId, amount: u128 },
    WithdrawCollateral { user: ActorId, amount: u128 },
    Lend,
    Withdraw(u128),
    Liquidate(ActorId),
    GetUserInfo(ActorId),
    Pause,
    Resume,
    UpdateTvaraPrice(u128),
    UtilizationRate,
    ClaimInterest,
    AdminWithdrawFunds(u128),
    AdminWithdrawTreasury(u128),
    GetContractState,
}

#[derive(Encode, Decode, TypeInfo, Clone, Debug)]
pub enum LendingReply {
    UserInfo {
        collateral: u128,
        debt: u128,
        lender_balance: u128,
        tvara_price: u128,
        health_factor: u128,
        accrued_interest: u128,
        lender_interest_earned: u128,
    },
    UtilizationRate(u128),
    Success,
    Error(String),
    ContractState(crate::ContractState),
}
