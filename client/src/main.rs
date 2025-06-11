#![allow(dead_code)]
use anchor_client::{Client, Cluster};
use anyhow::{format_err, Result};
use clap::Parser;
use configparser::ini::Ini;
use cpmm_lp_lock::{
    states::{LP_LOCK_COUNTER_SEED, USER_LOCK_SEED},
    LP_LOCK_VAULT_SEED,
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address_with_program_id;
use std::str::FromStr;
use std::{ops::Add, rc::Rc};

mod instructions;
use instructions::amm_instructions::*;
use instructions::rpc::*;

#[derive(Clone, Debug, PartialEq)]
pub struct ClientConfig {
    http_url: String,
    ws_url: String,
    payer_path: String,
    admin_path: String,
    raydium_cp_program: Pubkey,
    slippage: f64,
}

fn load_cfg(client_config: &String) -> Result<ClientConfig> {
    let mut config = Ini::new();
    let _map = config.load(client_config).unwrap();
    let http_url = config.get("Global", "http_url").unwrap();
    if http_url.is_empty() {
        panic!("http_url must not be empty");
    }
    let ws_url = config.get("Global", "ws_url").unwrap();
    if ws_url.is_empty() {
        panic!("ws_url must not be empty");
    }
    let payer_path = config.get("Global", "payer_path").unwrap();
    if payer_path.is_empty() {
        panic!("payer_path must not be empty");
    }
    let admin_path = config.get("Global", "admin_path").unwrap();
    if admin_path.is_empty() {
        panic!("admin_path must not be empty");
    }

    let raydium_cp_program_str = config.get("Global", "raydium_cp_program").unwrap();
    if raydium_cp_program_str.is_empty() {
        panic!("raydium_cp_program must not be empty");
    }
    let raydium_cp_program = Pubkey::from_str(&raydium_cp_program_str).unwrap();
    let slippage = config.getfloat("Global", "slippage").unwrap().unwrap();

    Ok(ClientConfig {
        http_url,
        ws_url,
        payer_path,
        admin_path,
        raydium_cp_program,
        slippage,
    })
}

fn read_keypair_file(s: &str) -> Result<Keypair> {
    solana_sdk::signature::read_keypair_file(s)
        .map_err(|_| format_err!("failed to read keypair from {}", s))
}

#[derive(Debug, Parser)]
pub struct Opts {
    #[clap(subcommand)]
    pub command: RaydiumCpCommands,
}

#[derive(Debug, Parser)]
pub enum RaydiumCpCommands {
    LockLp {
        #[arg(long)]
        pool_id: Pubkey,
        #[arg(long)]
        amount: u64,
        #[arg(long)]
        duration: u64,
    },
    LockLpPermanently {
        #[arg(long)]
        pool_id: Pubkey,
        #[arg(long)]
        amount: u64,
    },
    UnlockLp {
        #[arg(long)]
        pool_id: Pubkey,
        #[arg(long)]
        lock_id: u64,
    },
    CollectFees {
        #[arg(long)]
        pool_id: Pubkey,
        #[arg(long)]
        lock_id: u64,
    },
}

fn main() -> Result<()> {
    let client_config = "client_config.ini";
    let pool_config = load_cfg(&client_config.to_string()).unwrap();
    // cluster params.
    let payer = read_keypair_file(&pool_config.payer_path)?;
    // solana rpc client
    let rpc_client = RpcClient::new(pool_config.http_url.to_string());

    // anchor client.
    let anchor_config = pool_config.clone();
    let url = Cluster::Custom(anchor_config.http_url, anchor_config.ws_url);
    let wallet = read_keypair_file(&pool_config.payer_path)?;
    let anchor_client = Client::new(url, Rc::new(wallet));
    let program = anchor_client.program(pool_config.raydium_cp_program)?;

    let opts = Opts::parse();
    match opts.command {
        RaydiumCpCommands::LockLp {
            pool_id,
            amount,
            duration,
        } => {
            let pool_state: cpmm_lp_lock::states::PoolState = program.account(pool_id)?;
            let lp_mint = pool_state.lp_mint;
            let owner_lp_token_account = spl_associated_token_account::get_associated_token_address(
                &payer.pubkey(),
                &lp_mint,
            );
            let (lp_lock_counter, _) = Pubkey::find_program_address(
                &[
                    LP_LOCK_COUNTER_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                ],
                &program.id(),
            );
            let lp_lock_counter_info: cpmm_lp_lock::states::LpLockCounter =
                program.account(lp_lock_counter).unwrap_or_default();
            let (user_lp_lock, _) = Pubkey::find_program_address(
                &[
                    USER_LOCK_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                    lp_lock_counter_info
                        .total_lock_count
                        .add(1)
                        .to_le_bytes()
                        .as_ref(),
                ],
                &program.id(),
            );

            let (lp_lock_vault, _) = Pubkey::find_program_address(
                &[
                    LP_LOCK_VAULT_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                    lp_lock_counter_info
                        .total_lock_count
                        .add(1)
                        .to_le_bytes()
                        .as_ref(),
                ],
                &program.id(),
            );
            let mut instructions = Vec::new();
            let lock_lp_instr = lock_lp_instr(
                &pool_config,
                pool_id,
                owner_lp_token_account,
                pool_state.lp_mint,
                lp_lock_counter,
                user_lp_lock,
                lp_lock_vault,
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                amount,
                duration,
            )?;
            instructions.extend(lock_lp_instr);
            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        RaydiumCpCommands::LockLpPermanently { pool_id, amount } => {
            let pool_state: cpmm_lp_lock::states::PoolState = program.account(pool_id)?;
            let lp_mint = pool_state.lp_mint;
            let owner_lp_token_account = spl_associated_token_account::get_associated_token_address(
                &payer.pubkey(),
                &lp_mint,
            );
            let (lp_lock_counter, _) = Pubkey::find_program_address(
                &[
                    LP_LOCK_COUNTER_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                ],
                &program.id(),
            );
            let lp_lock_counter_info: cpmm_lp_lock::states::LpLockCounter =
                program.account(lp_lock_counter).unwrap_or_default();
            let (user_lp_lock, _) = Pubkey::find_program_address(
                &[
                    USER_LOCK_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                    lp_lock_counter_info
                        .total_lock_count
                        .add(1)
                        .to_le_bytes()
                        .as_ref(),
                ],
                &program.id(),
            );

            let (lp_lock_vault, _) = Pubkey::find_program_address(
                &[
                    LP_LOCK_VAULT_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                    lp_lock_counter_info
                        .total_lock_count
                        .add(1)
                        .to_le_bytes()
                        .as_ref(),
                ],
                &program.id(),
            );
            let mut instructions = Vec::new();
            let lock_lp_instr = lock_lp_perm_instr(
                &pool_config,
                pool_id,
                owner_lp_token_account,
                pool_state.lp_mint,
                lp_lock_counter,
                user_lp_lock,
                lp_lock_vault,
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                amount,
            )?;
            instructions.extend(lock_lp_instr);
            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        RaydiumCpCommands::UnlockLp { pool_id, lock_id } => {
            let pool_state: cpmm_lp_lock::states::PoolState = program.account(pool_id)?;
            let lp_mint = pool_state.lp_mint;
            let owner_lp_token_account = spl_associated_token_account::get_associated_token_address(
                &payer.pubkey(),
                &lp_mint,
            );
            let (lp_lock_counter, _) = Pubkey::find_program_address(
                &[
                    LP_LOCK_COUNTER_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                ],
                &program.id(),
            );
            let (user_lp_lock, _) = Pubkey::find_program_address(
                &[
                    USER_LOCK_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                    lock_id.to_le_bytes().as_ref(),
                ],
                &program.id(),
            );

            let (lp_lock_vault, _) = Pubkey::find_program_address(
                &[
                    LP_LOCK_VAULT_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                    lock_id.to_le_bytes().as_ref(),
                ],
                &program.id(),
            );
            let mut instructions = Vec::new();
            let lock_lp_instr = unlock_lp_instr(
                &pool_config,
                pool_id,
                owner_lp_token_account,
                pool_state.lp_mint,
                lp_lock_counter,
                user_lp_lock,
                lp_lock_vault,
            )?;
            instructions.extend(lock_lp_instr);
            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        RaydiumCpCommands::CollectFees { pool_id, lock_id } => {
            let pool_state: cpmm_lp_lock::states::PoolState = program.account(pool_id)?;
            let lp_mint = pool_state.lp_mint;
            let owner_lp_token_account = spl_associated_token_account::get_associated_token_address(
                &payer.pubkey(),
                &lp_mint,
            );
            let (lp_lock_counter, _) = Pubkey::find_program_address(
                &[
                    LP_LOCK_COUNTER_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                ],
                &program.id(),
            );
            let (user_lp_lock, _) = Pubkey::find_program_address(
                &[
                    USER_LOCK_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                    lock_id.to_le_bytes().as_ref(),
                ],
                &program.id(),
            );

            let (lp_lock_vault, _) = Pubkey::find_program_address(
                &[
                    LP_LOCK_VAULT_SEED.as_bytes(),
                    &payer.pubkey().as_ref(),
                    lp_mint.as_ref(),
                    lock_id.to_le_bytes().as_ref(),
                ],
                &program.id(),
            );
            let mut instructions = Vec::new();
            let lock_lp_instr = collect_fees_instr(
                &pool_config,
                pool_id,
                owner_lp_token_account,
                pool_state.lp_mint,
                lp_lock_counter,
                user_lp_lock,
                lp_lock_vault,
                get_associated_token_address_with_program_id(
                    &payer.pubkey(),
                    &pool_state.token_0_mint,
                    &pool_state.token_0_program,
                ),
                get_associated_token_address_with_program_id(
                    &payer.pubkey(),
                    &pool_state.token_1_mint,
                    &pool_state.token_1_program,
                ),
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
            )?;
            instructions.extend(lock_lp_instr);
            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
    }
    Ok(())
}
