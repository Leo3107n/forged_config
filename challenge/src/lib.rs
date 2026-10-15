use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
};

entrypoint!(process_instruction);

const WITHDRAW_ALL: u8 = 0;
const WITHDRAW_AMOUNT: u8 = 1;
const GET_TREASURY_BALANCE: u8 = 2;
const GET_CONFIG_AUTHORITY: u8 = 3;
const ROTATE_CONFIG_AUTHORITY: u8 = 4;
const GET_CANONICAL_ADDRESSES: u8 = 5;
const CONFIG_SEED: &[u8] = b"config";
const TREASURY_SEED: &[u8] = b"treasury";
const LEGACY_CONFIG_LEN: usize = 32;

enum TreasuryInstruction {
    WithdrawAll,
    WithdrawAmount { lamports: u64 },
    GetTreasuryBalance,
    GetConfigAuthority,
    RotateConfigAuthority { new_authority: Pubkey },
    GetCanonicalAddresses,
}

impl TreasuryInstruction {
    fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        match input {
            [WITHDRAW_ALL] => Ok(Self::WithdrawAll),
            [WITHDRAW_AMOUNT, amount @ ..] if amount.len() == 8 => {
                let lamports = u64::from_le_bytes(
                    amount
                        .try_into()
                        .map_err(|_| ProgramError::InvalidInstructionData)?,
                );
                Ok(Self::WithdrawAmount { lamports })
            }
            [GET_TREASURY_BALANCE] => Ok(Self::GetTreasuryBalance),
            [GET_CONFIG_AUTHORITY] => Ok(Self::GetConfigAuthority),
            [ROTATE_CONFIG_AUTHORITY, authority @ ..] if authority.len() == 32 => {
                let bytes: [u8; 32] = authority
                    .try_into()
                    .map_err(|_| ProgramError::InvalidInstructionData)?;
                Ok(Self::RotateConfigAuthority {
                    new_authority: Pubkey::new_from_array(bytes),
                })
            }
            [GET_CANONICAL_ADDRESSES] => Ok(Self::GetCanonicalAddresses),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

fn config_address(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED], program_id).0
}

fn treasury_address(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[TREASURY_SEED], program_id).0
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match TreasuryInstruction::unpack(instruction_data)? {
        TreasuryInstruction::WithdrawAll => withdraw(program_id, accounts, None),
        TreasuryInstruction::WithdrawAmount { lamports } => {
            withdraw(program_id, accounts, Some(lamports))
        }
        TreasuryInstruction::GetTreasuryBalance => get_treasury_balance(program_id, accounts),
        TreasuryInstruction::GetConfigAuthority => get_config_authority(accounts),
        TreasuryInstruction::RotateConfigAuthority { new_authority } => {
            rotate_config_authority(accounts, new_authority)
        }
        TreasuryInstruction::GetCanonicalAddresses => {
            let mut addresses = [0u8; 64];
            addresses[..32].copy_from_slice(config_address(program_id).as_ref());
            addresses[32..].copy_from_slice(treasury_address(program_id).as_ref());
            set_return_data(&addresses);
            Ok(())
        }
    }
}

fn withdraw(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    requested_lamports: Option<u64>,
) -> ProgramResult {
    let mut iter = accounts.iter();
    let config = next_account_info(&mut iter)?;
    let authority = next_account_info(&mut iter)?;
    let treasury = next_account_info(&mut iter)?;

    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !authority.is_writable || !treasury.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }
    validate_treasury(program_id, treasury)?;

    let stored_authority = read_config_authority(config)?;
    if stored_authority != *authority.key {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Vulnerability: this authorization path never authenticates config provenance.
    // A production service must require both of these before reading its bytes:
    // config.key == config_address(program_id)
    // config.owner == program_id
    let available = treasury.lamports();
    let amount = requested_lamports.unwrap_or(available);
    if amount == 0 || amount > available {
        return Err(ProgramError::InsufficientFunds);
    }

    **treasury.try_borrow_mut_lamports()? = available
        .checked_sub(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    **authority.try_borrow_mut_lamports()? = authority
        .lamports()
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    Ok(())
}

fn get_treasury_balance(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let treasury = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
    validate_treasury(program_id, treasury)?;
    set_return_data(&treasury.lamports().to_le_bytes());
    Ok(())
}

fn get_config_authority(accounts: &[AccountInfo]) -> ProgramResult {
    let config = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
    set_return_data(read_config_authority(config)?.as_ref());
    Ok(())
}

fn rotate_config_authority(accounts: &[AccountInfo], new_authority: Pubkey) -> ProgramResult {
    let mut iter = accounts.iter();
    let config = next_account_info(&mut iter)?;
    let current_authority = next_account_info(&mut iter)?;
    if !current_authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !config.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }
    if read_config_authority(config)? != *current_authority.key {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut data = config.try_borrow_mut_data()?;
    data[..LEGACY_CONFIG_LEN].copy_from_slice(new_authority.as_ref());
    Ok(())
}

fn validate_treasury(program_id: &Pubkey, treasury: &AccountInfo) -> ProgramResult {
    if treasury.key != &treasury_address(program_id) || treasury.owner != program_id {
        return Err(ProgramError::InvalidArgument);
    }
    Ok(())
}

fn read_config_authority(config: &AccountInfo) -> Result<Pubkey, ProgramError> {
    let data = config.try_borrow_data()?;
    if data.len() < LEGACY_CONFIG_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    Pubkey::try_from(&data[..LEGACY_CONFIG_LEN]).map_err(|_| ProgramError::InvalidAccountData)
}
