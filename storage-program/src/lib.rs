use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
    system_instruction,
};

entrypoint!(process_instruction);

#[derive(BorshSerialize, BorshDeserialize)]
pub struct ResultAccount {
    pub num1: u64,
    pub num2: u64,
    pub result: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum ResultInstruction {
    Initialize { num1: u64, num2: u64 },
    Update { num1: u64, num2: u64 },  
}

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let [payer, result_account, system_program_info] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    match ResultInstruction::try_from_slice(instruction_data)? {
        ResultInstruction::Initialize { num1, num2 } => {
            let sum = num1.checked_add(num2)
                .ok_or(ProgramError::InvalidArgument)?;
            
            msg!("Initializing: {} + {} = {}", num1, num2, sum);

            // Get the rent sysvar
            let account_size = result_account_data.try_to_vec()?.len();
            let lamports = Rent::get()?.minimum_balance(account_size);

            // Create account
            let create_ix = system_instruction::create_account(
                payer.key,
                result_account.key,
                lamports,
                account_size as u64,  // Space
                program_id,
            );
            
            invoke(&create_ix, &[
                payer.clone(),
                result_account.clone(),
                system_program_info.clone(),
            ])?;
           
            // Store the calculated result
            let result_account_data = ResultAccount {
                num1,
                num2,
                result: sum,
            };
            result_account_data.serialize(&mut *result_account.data.borrow_mut())?;
        }
        
        ResultInstruction::Update { num1, num2 } => {
            let sum = num1.checked_add(num2)
                .ok_or(ProgramError::InvalidArgument)?;
            
            msg!("Updating: {} + {} = {}", num1, num2, sum);
            
            // Read existing data
            let mut result_account_data = ResultAccount::try_from_slice(&result_account.data.borrow())?;
            
            // Update with new calculation
            result_account_data.num1 = num1;
            result_account_data.num2 = num2;
            result_account_data.result = sum;
            
            // Write back
            result_account_data.serialize(&mut *result_account.data.borrow_mut())?;
        }
    }

    Ok(())
}