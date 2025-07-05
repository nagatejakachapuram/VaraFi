#[warn(unused_variables)]
use blockchain_app::io::*;
use sails_rs::gtest::{Program, System};
use sails_rs::prelude::*;

const USERS: &[u64] = &[3, 4, 5, 6, 7, 8];
const VFT_ADDRESS: u64 = 2;

#[test]
fn test_init() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to admin user
    sys.mint_to(USERS[0], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    // Note: In a real test, you'd check the result status
}

#[test]
fn test_deposit_collateral() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let deposit_amount = 1_000_000_000_000; // 1 TVARA
    let before_balance = sys.balance_of(USERS[1]);
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    let after_balance = sys.balance_of(USERS[1]);
    assert_eq!(
        before_balance - after_balance,
        deposit_amount,
        "User's balance should decrease by deposit amount"
    );
    // Optionally, check that admin's balance is unchanged
    let admin_balance = sys.balance_of(USERS[0]);
    assert_eq!(
        admin_balance, 1_000_000_000_000_000,
        "Admin's balance should remain unchanged"
    );
    // let _user_info = lending_program.send(USERS[1], LendingAction::GetUserInfo(USERS[1].into()));
}

#[test]
fn test_borrow() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let deposit_amount = 1_000_000_000_000; // 1 TVARA
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    lending_program.send(USERS[1], LendingAction::Borrow);
}

#[test]
fn test_repay() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    lending_program.send(USERS[1], LendingAction::Borrow);
    let repay_amount = 500_000_000_000; // 0.5 TVARA
    lending_program.send(
        USERS[1],
        LendingAction::Repay {
            user: USERS[1].into(),
            amount: repay_amount,
        },
    );
}

#[test]
fn test_lend() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);
}

#[test]
fn test_withdraw_liquidity() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);
    let withdraw_amount = 500_000_000_000;
    lending_program.send(USERS[2], LendingAction::Withdraw(withdraw_amount));
}

#[test]
fn test_liquidate() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);
    sys.mint_to(USERS[3], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    lending_program.send(USERS[1], LendingAction::Borrow);
    lending_program.send(USERS[3], LendingAction::Liquidate(USERS[1].into()));
}

#[test]
fn test_health_factor() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    let _user_info = lending_program.send(USERS[1], LendingAction::GetUserInfo(USERS[1].into()));
    lending_program.send(USERS[1], LendingAction::Borrow);
    let _user_info_after =
        lending_program.send(USERS[1], LendingAction::GetUserInfo(USERS[1].into()));
}

#[test]
fn test_admin_functions() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to admin user
    sys.mint_to(USERS[0], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(USERS[0], LendingAction::Pause);
    lending_program.send(USERS[0], LendingAction::Resume);
}

#[test]
fn test_price_update() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to admin user
    sys.mint_to(USERS[0], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    let new_price = 1_100_000_000_000_000_000; // 1.1 USD per TVARA
    lending_program.send(USERS[0], LendingAction::UpdateTvaraPrice(new_price));
}

#[test]
fn test_utilization_rate() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);
    let _utilization = lending_program.send(USERS[0], LendingAction::UtilizationRate);
}

#[test]
fn test_interest_accrual() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);
    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    lending_program.send(USERS[1], LendingAction::Borrow);
    let _user_info = lending_program.send(USERS[1], LendingAction::GetUserInfo(USERS[1].into()));
}

#[test]
fn test_collateral_withdrawal() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    let withdraw_amount = 500_000_000_000;
    lending_program.send(
        USERS[1],
        LendingAction::WithdrawCollateral {
            user: USERS[1].into(),
            amount: withdraw_amount,
        },
    );
}

#[test]
fn test_reentrancy_protection() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
}

#[test]
fn test_edge_cases() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, 0);
    lending_program.send(USERS[1], LendingAction::Borrow);
    lending_program.send(
        USERS[1],
        LendingAction::Repay {
            user: USERS[1].into(),
            amount: 100_000_000_000,
        },
    );
}

#[test]
fn test_multiple_users() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to all users
    for user in USERS {
        sys.mint_to(*user, 1_000_000_000_000_000);
    }

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );
    for user in &USERS[1..] {
        let deposit_amount = 1_000_000_000_000;
        lending_program.send_with_value(*user, LendingAction::DepositCollateral, deposit_amount);
    }
    for user in &USERS[1..] {
        let lend_amount = 500_000_000_000;
        lending_program.send_with_value(*user, LendingAction::Lend, lend_amount);
    }
}

#[test]
fn test_insufficient_collateral_borrow() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Try to borrow without depositing collateral
    lending_program.send(USERS[1], LendingAction::Borrow);
}

#[test]
fn test_over_borrow_limit() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Provide some liquidity
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);

    // Deposit collateral
    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);

    // Try to borrow more than available liquidity
    lending_program.send(USERS[1], LendingAction::Borrow);
}

#[test]
fn test_repay_more_than_debt() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Setup: provide liquidity and borrow
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);

    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    lending_program.send(USERS[1], LendingAction::Borrow);

    // Try to repay more than the debt
    let excessive_repay = 10_000_000_000_000; // 10x more than borrowed
    lending_program.send(
        USERS[1],
        LendingAction::Repay {
            user: USERS[1].into(),
            amount: excessive_repay,
        },
    );
}

#[test]
fn test_withdraw_more_than_liquidity() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Provide liquidity
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);

    // Try to withdraw more than provided
    let excessive_withdraw = 10_000_000_000_000;
    lending_program.send(USERS[2], LendingAction::Withdraw(excessive_withdraw));
}

#[test]
fn test_withdraw_collateral_with_debt() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Setup: provide liquidity, deposit collateral, and borrow
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);

    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    lending_program.send(USERS[1], LendingAction::Borrow);

    // Try to withdraw collateral while having debt
    let withdraw_amount = 500_000_000_000;
    lending_program.send(
        USERS[1],
        LendingAction::WithdrawCollateral {
            user: USERS[1].into(),
            amount: withdraw_amount,
        },
    );
}

#[test]
fn test_liquidate_healthy_position() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);
    sys.mint_to(USERS[3], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Setup: deposit collateral but don't borrow (healthy position)
    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);

    // Try to liquidate a healthy position
    lending_program.send(USERS[3], LendingAction::Liquidate(USERS[1].into()));
}

#[test]
fn test_price_manipulation() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Setup: provide liquidity, deposit collateral, and borrow
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);

    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    lending_program.send(USERS[1], LendingAction::Borrow);

    // Dramatically increase price to make position unhealthy
    let high_price = 10_000_000_000_000_000_000; // 10x price increase
    lending_program.send(USERS[0], LendingAction::UpdateTvaraPrice(high_price));

    // Check if position is now liquidatable
    let _user_info = lending_program.send(USERS[1], LendingAction::GetUserInfo(USERS[1].into()));
}

#[test]
fn test_zero_price_update() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to admin user
    sys.mint_to(USERS[0], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Try to set price to zero (should fail)
    lending_program.send(USERS[0], LendingAction::UpdateTvaraPrice(0));
}

#[test]
fn test_non_admin_price_update() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Try to update price as non-admin user
    let new_price = 1_100_000_000_000_000_000;
    lending_program.send(USERS[1], LendingAction::UpdateTvaraPrice(new_price));
}

#[test]
fn test_pause_and_resume_operations() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Pause the protocol
    lending_program.send(USERS[0], LendingAction::Pause);

    // Try to perform operations while paused
    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);

    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);

    // Resume the protocol
    lending_program.send(USERS[0], LendingAction::Resume);

    // Try operations again after resume
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);
}

#[test]
fn test_multiple_borrows_and_repays() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Provide liquidity
    let lend_amount = 5_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);

    // Deposit collateral
    let deposit_amount = 2_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);

    // Multiple borrows
    lending_program.send(USERS[1], LendingAction::Borrow);
    lending_program.send(USERS[1], LendingAction::Borrow);
    lending_program.send(USERS[1], LendingAction::Borrow);

    // Multiple partial repays
    let repay_amount = 500_000_000_000;
    lending_program.send(
        USERS[1],
        LendingAction::Repay {
            user: USERS[1].into(),
            amount: repay_amount,
        },
    );
    lending_program.send(
        USERS[1],
        LendingAction::Repay {
            user: USERS[1].into(),
            amount: repay_amount,
        },
    );

    // Check final state
    let _user_info = lending_program.send(USERS[1], LendingAction::GetUserInfo(USERS[1].into()));
}

#[test]
fn test_high_utilization_scenario() {
    let sys = System::new();
    sys.init_logger();

    // Mint a huge balance to all users
    for user in USERS {
        sys.mint_to(*user, 1_000_000_000_000_000_000);
    }

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Provide liquidity
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);

    // Multiple users deposit collateral and borrow to create high utilization
    for user in &USERS[1..] {
        let deposit_amount = 500_000_000_000;
        lending_program.send_with_value(*user, LendingAction::DepositCollateral, deposit_amount);
        lending_program.send(*user, LendingAction::Borrow);
    }

    // Check utilization rate
    let _utilization = lending_program.send(USERS[0], LendingAction::UtilizationRate);
}

#[test]
fn test_collateral_ratio_scenarios() {
    let sys = System::new();
    sys.init_logger();

    // Mint a much larger balance to all users
    for user in USERS {
        sys.mint_to(*user, 1_000_000_000_000_000_000);
    }

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Provide liquidity
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);

    // Test different collateral ratios
    let scenarios = [
        (1_000_000_000_000, "1:1 ratio"),
        (2_000_000_000_000, "2:1 ratio"),
        (500_000_000_000, "0.5:1 ratio"),
    ];

    for (deposit_amount, _description) in scenarios.iter() {
        lending_program.send_with_value(
            USERS[1],
            LendingAction::DepositCollateral,
            *deposit_amount,
        );
        lending_program.send(USERS[1], LendingAction::Borrow);

        let _user_info =
            lending_program.send(USERS[1], LendingAction::GetUserInfo(USERS[1].into()));

        // Repay and withdraw for next scenario
        lending_program.send(
            USERS[1],
            LendingAction::Repay {
                user: USERS[1].into(),
                amount: 1_000_000_000_000,
            },
        );
        lending_program.send(
            USERS[1],
            LendingAction::WithdrawCollateral {
                user: USERS[1].into(),
                amount: *deposit_amount,
            },
        );
    }
}

#[test]
fn test_protocol_pause_during_active_positions() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to users
    sys.mint_to(USERS[0], 1_000_000_000_000_000);
    sys.mint_to(USERS[1], 1_000_000_000_000_000);
    sys.mint_to(USERS[2], 1_000_000_000_000_000);

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Setup active positions
    let lend_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[2], LendingAction::Lend, lend_amount);

    let deposit_amount = 1_000_000_000_000;
    lending_program.send_with_value(USERS[1], LendingAction::DepositCollateral, deposit_amount);
    lending_program.send(USERS[1], LendingAction::Borrow);

    // Pause protocol
    lending_program.send(USERS[0], LendingAction::Pause);

    // Try to interact with active positions while paused
    lending_program.send(
        USERS[1],
        LendingAction::Repay {
            user: USERS[1].into(),
            amount: 100_000_000_000,
        },
    );

    lending_program.send(USERS[2], LendingAction::Withdraw(100_000_000_000));

    // Resume and try again
    lending_program.send(USERS[0], LendingAction::Resume);

    lending_program.send(
        USERS[1],
        LendingAction::Repay {
            user: USERS[1].into(),
            amount: 100_000_000_000,
        },
    );

    lending_program.send(USERS[2], LendingAction::Withdraw(100_000_000_000));
}

#[test]
fn test_concurrent_user_operations() {
    let sys = System::new();
    sys.init_logger();

    // Mint balance to all users
    for user in USERS {
        sys.mint_to(*user, 1_000_000_000_000_000);
    }

    let lending_program = Program::current(&sys);
    lending_program.send(
        USERS[0],
        LendingInit {
            vft_address: VFT_ADDRESS.into(),
        },
    );

    // Simulate concurrent operations from multiple users
    let operations = [
        (
            USERS[1],
            LendingAction::DepositCollateral,
            1_000_000_000_000,
        ),
        (USERS[2], LendingAction::Lend, 1_000_000_000_000),
        (USERS[3], LendingAction::DepositCollateral, 500_000_000_000),
        (USERS[4], LendingAction::Lend, 500_000_000_000),
        (USERS[5], LendingAction::DepositCollateral, 750_000_000_000),
    ];

    // Execute operations in sequence (simulating concurrent access)
    for (user, action, amount) in operations.iter() {
        match action {
            LendingAction::DepositCollateral => {
                lending_program.send_with_value(*user, LendingAction::DepositCollateral, *amount);
            }
            LendingAction::Lend => {
                lending_program.send_with_value(*user, LendingAction::Lend, *amount);
            }
            _ => {}
        }
    }

    // Now have users borrow
    lending_program.send(USERS[1], LendingAction::Borrow);
    lending_program.send(USERS[3], LendingAction::Borrow);
    lending_program.send(USERS[5], LendingAction::Borrow);

    // Check final state
    for user in &USERS[1..] {
        let _user_info = lending_program.send(*user, LendingAction::GetUserInfo((*user).into()));
    }
}
