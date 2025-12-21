use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

// System Program ID
const SYSTEM_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("11111111111111111111111111111111");

// Program ID from lib.rs
const PROGRAM_ID: Pubkey = solana_sdk::pubkey!("22222222222222222222222222222222222222222222");

/// Helper function to find vault PDA
fn find_vault_pda(owner: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", owner.as_ref()], &PROGRAM_ID)
}

/// Helper function to create deposit instruction
fn create_deposit_instruction(owner: Pubkey, vault: Pubkey, amount: u64) -> Instruction {
    let mut instruction_data = vec![0u8]; // Discriminator for Deposit
    instruction_data.extend_from_slice(&amount.to_le_bytes());

    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(owner, true),           // owner (signer, writable)
            AccountMeta::new(vault, false),          // vault (writable)
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false), // system program
        ],
        data: instruction_data,
    }
}

/// Helper function to create withdraw instruction
fn create_withdraw_instruction(owner: Pubkey, vault: Pubkey) -> Instruction {
    let instruction_data = vec![1u8]; // Discriminator for Withdraw

    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(owner, true),           // owner (signer, writable)
            AccountMeta::new(vault, false),          // vault (writable)
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false), // system program
        ],
        data: instruction_data,
    }
}

#[test]
fn test_deposit_success() {
    // Setup LiteSVM
    let mut svm = LiteSVM::new();
    
    // Load the program
    let program_bytes =
        std::fs::read("target/release/libblueshift_vault.so").expect("Failed to read program file");
    svm.add_program(PROGRAM_ID, &program_bytes)
        .expect("Failed to add program");

    // Create owner keypair and fund it
    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 10 * LAMPORTS_PER_SOL)
        .expect("Failed to airdrop");

    // Find vault PDA
    let (vault_pda, _bump) = find_vault_pda(&owner.pubkey());

    // Get initial balances
    let owner_initial_balance = svm
        .get_account(&owner.pubkey())
        .expect("Owner account should exist")
        .lamports;

    // Create deposit instruction
    let deposit_amount = 2 * LAMPORTS_PER_SOL;
    let deposit_ix = create_deposit_instruction(owner.pubkey(), vault_pda, deposit_amount);

    // Create and send transaction
    let recent_blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[deposit_ix],
        Some(&owner.pubkey()),
        &[&owner],
        recent_blockhash,
    );

    let tx_result = svm.send_transaction(tx);
    assert!(tx_result.is_ok(), "Deposit transaction should succeed");

    // Verify vault balance
    let vault_account = svm
        .get_account(&vault_pda)
        .expect("Vault account should exist");
    assert_eq!(
        vault_account.lamports, deposit_amount,
        "Vault should contain the deposited amount"
    );

    // Verify owner balance decreased
    let owner_final_balance = svm
        .get_account(&owner.pubkey())
        .expect("Owner account should exist")
        .lamports;
    
    // Owner balance should decrease by at least the deposit amount (plus transaction fees)
    assert!(
        owner_final_balance < owner_initial_balance - deposit_amount,
        "Owner balance should decrease"
    );
}

#[test]
fn test_deposit_with_zero_amount_fails() {
    let mut svm = LiteSVM::new();
    let program_bytes =
        std::fs::read("target/release/libblueshift_vault.so").expect("Failed to read program file");
    svm.add_program(PROGRAM_ID, &program_bytes);

    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 10 * LAMPORTS_PER_SOL)
        .expect("Failed to airdrop");

    let (vault_pda, _bump) = find_vault_pda(&owner.pubkey());

    // Try to deposit zero lamports
    let deposit_ix = create_deposit_instruction(owner.pubkey(), vault_pda, 0);

    let recent_blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[deposit_ix],
        Some(&owner.pubkey()),
        &[&owner],
        recent_blockhash,
    );

    let tx_result = svm.send_transaction(tx);
    assert!(
        tx_result.is_err(),
        "Deposit with zero amount should fail"
    );
}

#[test]
fn test_deposit_non_empty_vault_fails() {
    let mut svm = LiteSVM::new();
    let program_bytes =
        std::fs::read("target/release/libblueshift_vault.so").expect("Failed to read program file");
    svm.add_program(PROGRAM_ID, &program_bytes);

    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 10 * LAMPORTS_PER_SOL)
        .expect("Failed to airdrop");

    let (vault_pda, _bump) = find_vault_pda(&owner.pubkey());

    // First deposit - should succeed
    let deposit_amount = 2 * LAMPORTS_PER_SOL;
    let deposit_ix = create_deposit_instruction(owner.pubkey(), vault_pda, deposit_amount);

    let recent_blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[deposit_ix],
        Some(&owner.pubkey()),
        &[&owner],
        recent_blockhash,
    );

    svm.send_transaction(tx)
        .expect("First deposit should succeed");

    // Second deposit - should fail because vault is not empty
    let deposit_ix2 = create_deposit_instruction(owner.pubkey(), vault_pda, deposit_amount);

    let recent_blockhash = svm.latest_blockhash();
    let tx2 = Transaction::new_signed_with_payer(
        &[deposit_ix2],
        Some(&owner.pubkey()),
        &[&owner],
        recent_blockhash,
    );

    let tx_result = svm.send_transaction(tx2);
    assert!(
        tx_result.is_err(),
        "Second deposit should fail when vault is not empty"
    );
}

#[test]
fn test_withdraw_success() {
    let mut svm = LiteSVM::new();
    let program_bytes =
        std::fs::read("target/release/libblueshift_vault.so").expect("Failed to read program file");
    svm.add_program(PROGRAM_ID, &program_bytes);

    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 10 * LAMPORTS_PER_SOL)
        .expect("Failed to airdrop");

    let (vault_pda, _bump) = find_vault_pda(&owner.pubkey());

    // First, deposit some lamports
    let deposit_amount = 2 * LAMPORTS_PER_SOL;
    let deposit_ix = create_deposit_instruction(owner.pubkey(), vault_pda, deposit_amount);

    let recent_blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[deposit_ix],
        Some(&owner.pubkey()),
        &[&owner],
        recent_blockhash,
    );

    svm.send_transaction(tx).expect("Deposit should succeed");

    // Get owner balance before withdrawal
    let owner_balance_before = svm
        .get_account(&owner.pubkey())
        .expect("Owner account should exist")
        .lamports;

    // Now withdraw
    let withdraw_ix = create_withdraw_instruction(owner.pubkey(), vault_pda);

    let recent_blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[withdraw_ix],
        Some(&owner.pubkey()),
        &[&owner],
        recent_blockhash,
    );

    let tx_result = svm.send_transaction(tx);
    assert!(tx_result.is_ok(), "Withdraw transaction should succeed");

    // Verify vault is empty (it may still exist with 0 lamports or may not exist at all)
    let vault_account = svm.get_account(&vault_pda);
    if let Some(account) = vault_account {
        assert_eq!(
            account.lamports, 0,
            "Vault should be empty after withdrawal"
        );
    }
    // If vault doesn't exist, that's also valid - it means all lamports were withdrawn

    // Verify owner balance increased
    let owner_balance_after = svm
        .get_account(&owner.pubkey())
        .expect("Owner account should exist")
        .lamports;

    assert!(
        owner_balance_after > owner_balance_before,
        "Owner balance should increase after withdrawal"
    );

    // Owner should have received approximately the deposited amount (minus fees)
    assert!(
        owner_balance_after >= owner_balance_before + deposit_amount - 10000,
        "Owner should receive approximately the deposited amount"
    );
}

#[test]
fn test_withdraw_empty_vault_fails() {
    let mut svm = LiteSVM::new();
    let program_bytes =
        std::fs::read("target/release/libblueshift_vault.so").expect("Failed to read program file");
    svm.add_program(PROGRAM_ID, &program_bytes);

    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 10 * LAMPORTS_PER_SOL)
        .expect("Failed to airdrop");

    let (vault_pda, _bump) = find_vault_pda(&owner.pubkey());

    // Try to withdraw from empty vault
    let withdraw_ix = create_withdraw_instruction(owner.pubkey(), vault_pda);

    let recent_blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[withdraw_ix],
        Some(&owner.pubkey()),
        &[&owner],
        recent_blockhash,
    );

    let tx_result = svm.send_transaction(tx);
    assert!(
        tx_result.is_err(),
        "Withdraw from empty vault should fail"
    );
}

#[test]
fn test_withdraw_unauthorized_fails() {
    let mut svm = LiteSVM::new();
    let program_bytes =
        std::fs::read("target/release/libblueshift_vault.so").expect("Failed to read program file");
    svm.add_program(PROGRAM_ID, &program_bytes);

    let owner = Keypair::new();
    let attacker = Keypair::new();
    
    svm.airdrop(&owner.pubkey(), 10 * LAMPORTS_PER_SOL)
        .expect("Failed to airdrop to owner");
    svm.airdrop(&attacker.pubkey(), 10 * LAMPORTS_PER_SOL)
        .expect("Failed to airdrop to attacker");

    let (vault_pda, _bump) = find_vault_pda(&owner.pubkey());

    // Owner deposits
    let deposit_amount = 2 * LAMPORTS_PER_SOL;
    let deposit_ix = create_deposit_instruction(owner.pubkey(), vault_pda, deposit_amount);

    let recent_blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[deposit_ix],
        Some(&owner.pubkey()),
        &[&owner],
        recent_blockhash,
    );

    svm.send_transaction(tx).expect("Deposit should succeed");

    // Attacker tries to withdraw from owner's vault
    let withdraw_ix = create_withdraw_instruction(attacker.pubkey(), vault_pda);

    let recent_blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[withdraw_ix],
        Some(&attacker.pubkey()),
        &[&attacker],
        recent_blockhash,
    );

    let tx_result = svm.send_transaction(tx);
    assert!(
        tx_result.is_err(),
        "Attacker should not be able to withdraw from owner's vault"
    );
}

#[test]
fn test_deposit_and_withdraw_full_cycle() {
    let mut svm = LiteSVM::new();
    let program_bytes =
        std::fs::read("target/release/libblueshift_vault.so").expect("Failed to read program file");
    svm.add_program(PROGRAM_ID, &program_bytes);

    let owner = Keypair::new();
    let initial_airdrop = 10 * LAMPORTS_PER_SOL;
    svm.airdrop(&owner.pubkey(), initial_airdrop)
        .expect("Failed to airdrop");

    let (vault_pda, _bump) = find_vault_pda(&owner.pubkey());

    // Deposit
    let deposit_amount = 5 * LAMPORTS_PER_SOL;
    let deposit_ix = create_deposit_instruction(owner.pubkey(), vault_pda, deposit_amount);

    let recent_blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[deposit_ix],
        Some(&owner.pubkey()),
        &[&owner],
        recent_blockhash,
    );

    svm.send_transaction(tx).expect("Deposit should succeed");

    // Verify deposit
    let vault_balance = svm
        .get_account(&vault_pda)
        .expect("Vault should exist")
        .lamports;
    assert_eq!(vault_balance, deposit_amount);

    // Withdraw
    let withdraw_ix = create_withdraw_instruction(owner.pubkey(), vault_pda);

    let recent_blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[withdraw_ix],
        Some(&owner.pubkey()),
        &[&owner],
        recent_blockhash,
    );

    svm.send_transaction(tx)
        .expect("Withdraw should succeed");

    // Verify withdrawal - vault may not exist or have 0 lamports
    let vault_balance_after = svm
        .get_account(&vault_pda)
        .map(|acc| acc.lamports)
        .unwrap_or(0);
    assert_eq!(vault_balance_after, 0, "Vault should be empty");

    let owner_final_balance = svm
        .get_account(&owner.pubkey())
        .expect("Owner should exist")
        .lamports;

    // Owner should have close to the initial amount (minus transaction fees)
    // Allow for a small margin for transaction fees
    let expected_min_balance = initial_airdrop - 50000; // Allow up to 0.00005 SOL in fees
    assert!(
        owner_final_balance >= expected_min_balance,
        "Owner should get most of their funds back (fees: {} lamports)",
        initial_airdrop - owner_final_balance
    );
}