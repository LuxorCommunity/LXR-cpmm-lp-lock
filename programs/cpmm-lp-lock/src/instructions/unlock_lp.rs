use crate::error::ErrorCode;
use crate::states::*;
use crate::utils::token::*;
use crate::LP_LOCK_VAULT_SEED;
use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_interface::{Mint, TokenAccount};
use anchor_lang::{solana_program::clock};

#[derive(Accounts)]
pub struct UnlockLp<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: pool vault and lp mint authority
    #[account(
        address = crate::vault_and_lp_mint_auth::id()
    )]
    pub authority: UncheckedAccount<'info>,

    /// CHECK: lock vault authority
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
     bump,
    )]
    pub lock_vault_authority: UncheckedAccount<'info>,

    /// CHECK: Raydium pool state account
    #[account(
        owner = crate::raydium_cpmm::id()
    )]
    pub pool_state: UncheckedAccount<'info>,

    /// CHECK Owner lp tokan account
    #[account(
        mut,
        token::mint = lp_mint,
        token::authority = owner,
        token::token_program = token_program,  
    )]
    pub owner_lp_token: Box<InterfaceAccount<'info, TokenAccount>>,

    /// token Program
    pub token_program: Program<'info, Token>,

    /// Lp token mint
    #[account(
        mint::authority = authority,
    )]
    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [
            LP_LOCK_COUNTER_SEED.as_bytes(),
            owner.key().as_ref(),
            lp_mint.key().as_ref()
        ],
        bump,
    )]
    pub lp_lock_counter: Box<Account<'info, LpLockCounter>>,

    #[account(
        mut,
        constraint = user_lp_lock.user == owner.key(),
    )]
    pub user_lp_lock: Box<Account<'info, UserLock>>,

    /// CHECK The vault that holds the locked LP tokens
    #[account(
        mut , 
        token::mint = lp_mint, 
        token::authority = lock_vault_authority ,
        seeds = [
            LP_LOCK_VAULT_SEED.as_bytes(),
            owner.key().as_ref(),
            lp_mint.key().as_ref(),
            user_lp_lock.lock_count.to_le_bytes().as_ref()
        ],
        bump,
    )]
    pub lp_lock_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
}

pub fn unlock_lp(ctx: Context<UnlockLp>) -> Result<()> {
    let user_lock = &mut ctx.accounts.user_lp_lock;
    let lp_lock_counter = &mut ctx.accounts.lp_lock_counter;

    require_eq!(
        user_lock.is_locked_permanently,
        false,
        ErrorCode::LockIsPermanent
    );

    require_eq!(
        user_lock.is_unlocked,
        false,
        ErrorCode::LockAlreadyUnlocked
    );

    let block_timestamp: u64 = match clock::Clock::get() {
        Ok(clock) => match clock.unix_timestamp.try_into() {
            Ok(timestamp) => timestamp,
            Err(_) => {
                return Err(error!(ErrorCode::InvalidTimestamp));
            }
        },
        Err(_) => {
            return Err(error!(ErrorCode::ClockUnavailable));
        }
    };

    if block_timestamp < user_lock.unlock_time {
        return Err(error!(ErrorCode::UnlockTimeNotReached));
    }

    let pool_state_info = &ctx.accounts.pool_state;
    let pool_state = PoolState::try_deserialize(&mut &pool_state_info.data.borrow()[..])?;
    require_eq!(pool_state.lp_mint,ctx.accounts.lp_mint.key(), ErrorCode::IncorrectLpMint);

    // update user lock
    user_lock.is_unlocked = true;
    user_lock.last_updated = block_timestamp;

    // update lp lock counter
    lp_lock_counter.total_lock_amount -= user_lock.lock_amount;

    transfer_from_pool_vault_to_user(
        ctx.accounts.lock_vault_authority.to_account_info(),
        ctx.accounts.lp_lock_vault.to_account_info(),
        ctx.accounts.owner_lp_token.to_account_info(),
        ctx.accounts.lp_mint.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        user_lock.lock_amount,
        ctx.accounts.lp_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[ctx.bumps.lock_vault_authority]]],
    )?;

    emit!(
        LpUnlockEvent {
            user: user_lock.user,
            amount: user_lock.lock_amount,
            lp_mint: user_lock.lp_mint
        }
    );

    Ok(())
}
