use anchor_client::{Client, Cluster};
use anyhow::Ok;
use anyhow::Result;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, system_program};

use cpmm_lp_lock::accounts as raydium_cp_accounts;
use cpmm_lp_lock::instruction as raydium_cp_instructions;
use cpmm_lp_lock::raydium_cpmm;
use cpmm_lp_lock::vault_and_lp_mint_auth;
use cpmm_lp_lock::AUTH_SEED;
use std::rc::Rc;

use super::super::{read_keypair_file, ClientConfig};

pub fn lock_lp_instr(
    config: &ClientConfig,
    pool_id: Pubkey,
    user_token_lp_account: Pubkey,
    token_lp_mint: Pubkey,
    lp_lock_counter: Pubkey,
    user_lp_lock: Pubkey,
    lp_lock_vault: Pubkey,
    token_0_vault: Pubkey,
    token_1_vault: Pubkey,
    lp_token_amount: u64,
    lock_duration: u64,
) -> Result<Vec<Instruction>> {
    let payer = read_keypair_file(&config.payer_path)?;
    let url = Cluster::Custom(config.http_url.clone(), config.ws_url.clone());
    // Client.
    let client = Client::new(url, Rc::new(payer));
    let program = client.program(config.cpmm_lp_lock_program)?;

    let (lock_vault_authority, __bump) =
        Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &program.id());

    let instructions = program
        .request()
        .accounts(raydium_cp_accounts::LockLp {
            owner: program.payer(),
            authority: vault_and_lp_mint_auth::id(),
            lock_vault_authority,
            pool_state: pool_id,
            owner_lp_token: user_token_lp_account,
            token_program: spl_token::id(),
            lp_mint: token_lp_mint,
            lp_lock_counter,
            user_lp_lock,
            lp_lock_vault,
            token_0_vault,
            token_1_vault,
            system_program: system_program::id(),
        })
        .args(raydium_cp_instructions::LockLp {
            amount: lp_token_amount,
            lock_duration,
        })
        .instructions()?;
    Ok(instructions)
}

pub fn lock_lp_perm_instr(
    config: &ClientConfig,
    pool_id: Pubkey,
    user_token_lp_account: Pubkey,
    token_lp_mint: Pubkey,
    lp_lock_counter: Pubkey,
    user_lp_lock: Pubkey,
    lp_lock_vault: Pubkey,
    token_0_vault: Pubkey,
    token_1_vault: Pubkey,
    lp_token_amount: u64,
) -> Result<Vec<Instruction>> {
    let payer = read_keypair_file(&config.payer_path)?;
    let url = Cluster::Custom(config.http_url.clone(), config.ws_url.clone());
    // Client.
    let client = Client::new(url, Rc::new(payer));
    let program = client.program(config.cpmm_lp_lock_program)?;

    let (lock_vault_authority, __bump) =
        Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &program.id());

    let instructions = program
        .request()
        .accounts(raydium_cp_accounts::LockLp {
            owner: program.payer(),
            authority: vault_and_lp_mint_auth::id(),
            lock_vault_authority,
            pool_state: pool_id,
            owner_lp_token: user_token_lp_account,
            token_program: spl_token::id(),
            lp_mint: token_lp_mint,
            lp_lock_counter,
            user_lp_lock,
            lp_lock_vault,
            token_0_vault,
            token_1_vault,
            system_program: system_program::id(),
        })
        .args(raydium_cp_instructions::LockLpPermanent {
            amount: lp_token_amount,
        })
        .instructions()?;
    Ok(instructions)
}

pub fn unlock_lp_instr(
    config: &ClientConfig,
    pool_id: Pubkey,
    user_token_lp_account: Pubkey,
    token_lp_mint: Pubkey,
    lp_lock_counter: Pubkey,
    user_lp_lock: Pubkey,
    lp_lock_vault: Pubkey,
) -> Result<Vec<Instruction>> {
    let payer = read_keypair_file(&config.payer_path)?;
    let url = Cluster::Custom(config.http_url.clone(), config.ws_url.clone());
    // Client.
    let client = Client::new(url, Rc::new(payer));
    let program = client.program(config.cpmm_lp_lock_program)?;

    let (lock_vault_authority, __bump) =
        Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &program.id());

    let instructions = program
        .request()
        .accounts(raydium_cp_accounts::UnlockLp {
            owner: program.payer(),
            authority: vault_and_lp_mint_auth::id(),
            lock_vault_authority,
            pool_state: pool_id,
            owner_lp_token: user_token_lp_account,
            token_program: spl_token::id(),
            lp_mint: token_lp_mint,
            lp_lock_counter,
            user_lp_lock,
            lp_lock_vault,
            system_program: system_program::id(),
        })
        .args(raydium_cp_instructions::UnlockLp {})
        .instructions()?;
    Ok(instructions)
}

pub fn collect_fees_instr(
    config: &ClientConfig,
    pool_id: Pubkey,
    user_token_lp_account: Pubkey,
    token_lp_mint: Pubkey,
    lp_lock_counter: Pubkey,
    user_lp_lock: Pubkey,
    lp_lock_vault: Pubkey,
    token_0_account: Pubkey,
    token_1_account: Pubkey,
    token_0_vault: Pubkey,
    token_1_vault: Pubkey,
    vault_0_mint: Pubkey,
    vault_1_mint: Pubkey,
) -> Result<Vec<Instruction>> {
    let payer = read_keypair_file(&config.payer_path)?;
    let url = Cluster::Custom(config.http_url.clone(), config.ws_url.clone());
    // Client.
    let client = Client::new(url, Rc::new(payer));
    let program = client.program(config.cpmm_lp_lock_program)?;

    let (lock_vault_authority, __bump) =
        Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &program.id());

    let instructions = program
        .request()
        .accounts(raydium_cp_accounts::CollectFees {
            owner: program.payer(),
            authority: vault_and_lp_mint_auth::id(),
            lock_vault_authority,
            pool_state: pool_id,
            owner_lp_token: user_token_lp_account,
            lp_mint: token_lp_mint,
            lp_lock_counter,
            user_lp_lock,
            lp_lock_vault,
            token_0_account,
            token_1_account,
            token_0_vault,
            token_1_vault,
            token_program: spl_token::id(),
            token_program_2022: spl_token_2022::id(),
            vault_0_mint,
            vault_1_mint,
            memo_program: spl_memo::id(),
            raydium_cpmm_program: raydium_cpmm::id(),
            system_program: system_program::id(),
        })
        .args(raydium_cp_instructions::CollectFees {})
        .instructions()?;
    Ok(instructions)
}
