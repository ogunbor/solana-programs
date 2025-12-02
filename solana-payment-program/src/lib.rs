use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg,
    system_instruction,
    program::invoke_signed,
    rent::Rent,
    sysvar::Sysvar,
};

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub enum Instruction {
    Signup { name: String },
    Onramp { symbol: String, amount: u64 },
    Transfer { symbol: String, amount: u64, recipient: Pubkey },
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct UserAccount {
    pub signer: Pubkey,
    pub name: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BalanceAccount {
    pub owner: Pubkey,
    pub symbol: String,
    pub amount: u64,
}

entrypoint!(process_instruction);

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let ix: Instruction = Instruction::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    
    match ix {
        Instruction::Signup { name } => {
            msg!("Signup with name: {}", name);
            signup(program_id, accounts, name)?;
        },
        Instruction::Onramp { symbol, amount } => {
            msg!("Onramp {} {}", symbol, amount);
            onramp(program_id, accounts, symbol, amount)?;
        },
        Instruction::Transfer { symbol, amount, recipient } => {
            msg!("Transfer {} {} to {:?}", symbol, amount, recipient);
            transfer(program_id, accounts, symbol, amount, recipient)?;
        }
    }
    
    Ok(())
}

// PDA Seeds
const USER_SEED: &[u8] = b"user";
const BALANCE_SEED: &[u8] = b"balance";

fn signup(program_id: &Pubkey, accounts: &[AccountInfo], name: String) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    // Accounts:
    // 0. Signer authority (payer)
    // 1. User PDA account (to be created)
    // 2. System program
    let signer = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Derive PDA for user account
    let (user_pda, user_bump) = Pubkey::find_program_address(
        &[USER_SEED, signer.key.as_ref()],
        program_id,
    );
    
    if user_pda != *user_account.key {
        msg!("Invalid user PDA");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Create the user account
    let user_data = UserAccount {
        signer: *signer.key,
        name,
    };
    
    let account_size = user_data.try_to_vec()?.len();
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(account_size);
    
    invoke_signed(
        &system_instruction::create_account(
            signer.key,
            user_account.key,
            lamports,
            account_size as u64,
            program_id,
        ),
        &[signer.clone(), user_account.clone(), system_program.clone()],
        &[&[USER_SEED, signer.key.as_ref(), &[user_bump]]],
    )?;
    
    // Serialize user data into account
    user_data.serialize(&mut &mut user_account.data.borrow_mut()[..])?;
    
    msg!("User account created successfully");
    Ok(())
}

fn onramp(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    symbol: String,
    amount: u64,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    // Accounts:
    // 0. User authority (signer, payer)
    // 1. User PDA account
    // 2. Balance PDA account (to be created or updated)
    // 3. System program
    let signer = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;
    let balance_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify user account PDA
    let (user_pda, _) = Pubkey::find_program_address(
        &[USER_SEED, signer.key.as_ref()],
        program_id,
    );
    
    if user_pda != *user_account.key {
        msg!("Invalid user PDA");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Verify balance account PDA
    let (balance_pda, balance_bump) = Pubkey::find_program_address(
        &[BALANCE_SEED, signer.key.as_ref(), symbol.as_bytes()],
        program_id,
    );
    
    if balance_pda != *balance_account.key {
        msg!("Invalid balance PDA");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Check if balance account exists
    if balance_account.data_is_empty() {
        // Create new balance account
        let balance_data = BalanceAccount {
            owner: *signer.key,
            symbol: symbol.clone(),
            amount,
        };
        
        let account_size = balance_data.try_to_vec()?.len();
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(account_size);
        
        invoke_signed(
            &system_instruction::create_account(
                signer.key,
                balance_account.key,
                lamports,
                account_size as u64,
                program_id,
            ),
            &[signer.clone(), balance_account.clone(), system_program.clone()],
            &[&[BALANCE_SEED, signer.key.as_ref(), symbol.as_bytes(), &[balance_bump]]],
        )?;
        
        balance_data.serialize(&mut &mut balance_account.data.borrow_mut()[..])?;
        msg!("Balance account created with {} {}", amount, symbol);
    } else {
        // Update existing balance account
        let mut balance_data = BalanceAccount::try_from_slice(&balance_account.data.borrow())?;
        balance_data.amount = balance_data.amount.checked_add(amount)
            .ok_or(ProgramError::InvalidArgument)?;
        
        balance_data.serialize(&mut &mut balance_account.data.borrow_mut()[..])?;
        msg!("Balance updated to {} {}", balance_data.amount, symbol);
    }
    
    Ok(())
}

fn transfer(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    symbol: String,
    amount: u64,
    recipient: Pubkey,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    // Accounts:
    // 0. Sender authority (signer, payer)
    // 1. Sender balance PDA account
    // 2. Recipient balance PDA account (to be created or updated)
    // 3. System program
    let signer = next_account_info(account_iter)?;
    let sender_balance = next_account_info(account_iter)?;
    let recipient_balance = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify sender balance PDA
    let (sender_balance_pda, _) = Pubkey::find_program_address(
        &[BALANCE_SEED, signer.key.as_ref(), symbol.as_bytes()],
        program_id,
    );
    
    if sender_balance_pda != *sender_balance.key {
        msg!("Invalid sender balance PDA");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Verify recipient balance PDA
    let (recipient_balance_pda, recipient_bump) = Pubkey::find_program_address(
        &[BALANCE_SEED, recipient.as_ref(), symbol.as_bytes()],
        program_id,
    );
    
    if recipient_balance_pda != *recipient_balance.key {
        msg!("Invalid recipient balance PDA");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Deduct from sender
    let mut sender_data = BalanceAccount::try_from_slice(&sender_balance.data.borrow())?;
    if sender_data.amount < amount {
        msg!("Insufficient balance");
        return Err(ProgramError::InsufficientFunds);
    }
    
    sender_data.amount = sender_data.amount.checked_sub(amount)
        .ok_or(ProgramError::InvalidArgument)?;
    sender_data.serialize(&mut &mut sender_balance.data.borrow_mut()[..])?;
    
    // Add to recipient
    if recipient_balance.data_is_empty() {
        // Create new balance account for recipient
        let balance_data = BalanceAccount {
            owner: recipient,
            symbol: symbol.clone(),
            amount,
        };
        
        let account_size = balance_data.try_to_vec()?.len();
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(account_size);
        
        invoke_signed(
            &system_instruction::create_account(
                signer.key,
                recipient_balance.key,
                lamports,
                account_size as u64,
                program_id,
            ),
            &[signer.clone(), recipient_balance.clone(), system_program.clone()],
            &[&[BALANCE_SEED, recipient.as_ref(), symbol.as_bytes(), &[recipient_bump]]],
        )?;
        
        balance_data.serialize(&mut &mut recipient_balance.data.borrow_mut()[..])?;
    } else {
        // Update existing recipient balance
        let mut recipient_data = BalanceAccount::try_from_slice(&recipient_balance.data.borrow())?;
        recipient_data.amount = recipient_data.amount.checked_add(amount)
            .ok_or(ProgramError::InvalidArgument)?;
        
        recipient_data.serialize(&mut &mut recipient_balance.data.borrow_mut()[..])?;
    }
    
    msg!("Transferred {} {} from {:?} to {:?}", amount, symbol, signer.key, recipient);
    Ok(())
}

