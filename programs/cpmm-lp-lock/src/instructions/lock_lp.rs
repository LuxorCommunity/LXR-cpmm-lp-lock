use std::ops::Add;
use crate::curve::CurveCalculator;
use crate::curve::RoundDirection;
use crate::error::ErrorCode;
use crate::states::*;
use crate::utils::token::*;
use crate::utils::*;
use crate::LP_LOCK_VAULT_SEED;
use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_interface::{Mint, TokenAccount};
use anchor_lang::{solana_program::clock};

#[derive(Accounts)]
pub struct LockLp<'info> {
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
        init_if_needed,
        seeds = [
            LP_LOCK_COUNTER_SEED.as_bytes(),
            owner.key().as_ref(),
            lp_mint.key().as_ref()
        ],
        bump,
        payer = owner,
        space = LpLockCounter::LEN,
    )]
    pub lp_lock_counter: Box<Account<'info, LpLockCounter>>,

    #[account(
        init,
        seeds = [
            USER_LOCK_SEED.as_bytes(),
            owner.key().as_ref(),
            lp_mint.key().as_ref(),
            lp_lock_counter.total_lock_count.add(1).to_le_bytes().as_ref()
        ],
        bump,
        payer = owner,
        space = UserLock::LEN,
    )]
    pub user_lp_lock: Box<Account<'info, UserLock>>,

    /// CHECK The vault that holds the locked LP tokens
    #[account(
        mut,
        seeds = [
            LP_LOCK_VAULT_SEED.as_bytes(),
            owner.key().as_ref(),
            lp_mint.key().as_ref(),
            lp_lock_counter.total_lock_count.add(1).to_le_bytes().as_ref()
        ],
        bump,
    )]
    pub lp_lock_vault: UncheckedAccount<'info>,

    /// The address that holds pool tokens for token_0
    pub token_0_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The address that holds pool tokens for token_1
    pub token_1_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
}

pub fn lock_lp(
    ctx: Context<LockLp>,
    amount: u64,
    lock_duration: u64,
    lock_permanent: bool,
) -> Result<()> {
    require!(amount > 100,ErrorCode::InitLpAmountTooLess);

    if !lock_permanent {
        require!(
            lock_duration < 15_76_80_000,
            ErrorCode::LockDurationTooLong
        );
    }

    let lp_lock_counter = &mut ctx.accounts.lp_lock_counter;
    let user_lock = &mut ctx.accounts.user_lp_lock;

    // Check if lp lock counter is initialized in the same transaction
    if lp_lock_counter.user == Pubkey::default() {
        lp_lock_counter.user = ctx.accounts.owner.key();
        lp_lock_counter.lp_mint = ctx.accounts.lp_mint.key();
        lp_lock_counter.total_lock_count = 0;
        lp_lock_counter.total_lock_amount = 0;
    }

    let new_lock_count = lp_lock_counter.total_lock_count
        .checked_add(1)
        .ok_or(ErrorCode::Overflow)?;

    create_token_account(
        &ctx.accounts.lock_vault_authority.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.lp_lock_vault.to_account_info(),
        &ctx.accounts.lp_mint.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        &[&[
            LP_LOCK_VAULT_SEED.as_bytes(),
            ctx.accounts.owner.key().as_ref(),
            ctx.accounts.lp_mint.key().as_ref(),
            new_lock_count
                .to_le_bytes()
                .as_ref(),
            &[ctx.bumps.lp_lock_vault][..],
        ][..]],
    )?;

    transfer_from_user_to_pool_vault(
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.owner_lp_token.to_account_info(),
        ctx.accounts.lp_lock_vault.to_account_info(),
        ctx.accounts.lp_mint.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        amount,
        ctx.accounts.lp_mint.decimals,
    )?;

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

    let unlock_time = if lock_permanent {
        0
    } else {
        block_timestamp
            .checked_add(lock_duration)
            .ok_or(ErrorCode::Overflow)?
    };

    let pool_state_info = &ctx.accounts.pool_state;
    let pool_state = PoolState::try_deserialize(&mut &pool_state_info.data.borrow()[..])?;
    require_eq!(pool_state.lp_mint,ctx.accounts.lp_mint.key(), ErrorCode::IncorrectLpMint);
    require_eq!(pool_state.token_0_vault,ctx.accounts.token_0_vault.key());
    require_eq!(pool_state.token_1_vault,ctx.accounts.token_1_vault.key());

    let (total_token_0_amount, total_token_1_amount) = pool_state.vault_amount_without_fee(
        ctx.accounts.token_0_vault.amount,
        ctx.accounts.token_1_vault.amount,
    );

    let results = CurveCalculator::lp_tokens_to_trading_tokens(
        u128::from(amount),
        u128::from(pool_state.lp_supply),
        u128::from(total_token_0_amount),
        u128::from(total_token_1_amount),
        RoundDirection::Floor,
    )
    .ok_or(ErrorCode::ZeroTradingTokens)?;

    require_gt!(results.token_0_amount, 0);
    require_gt!(results.token_1_amount, 0);

    let liquidity = U128::from(results.token_0_amount)
        .checked_mul(results.token_1_amount.into())
        .unwrap()
        .integer_sqrt()
        .as_u64();

    user_lock.bump = ctx.bumps.user_lp_lock;
    user_lock.user = ctx.accounts.owner.key();
    user_lock.lp_mint = ctx.accounts.lp_mint.key();
    user_lock.lock_count = new_lock_count;
    user_lock.lock_amount = amount;
    user_lock.unlock_time = unlock_time;
    user_lock.principal_token_0 = results.token_0_amount.try_into().unwrap();
    user_lock.principal_token_1 = results.token_1_amount.try_into().unwrap();
    user_lock.principal_liquidity = liquidity;
    user_lock.is_locked_permanently = lock_permanent;
    user_lock.token_0_fees_collected = 0;
    user_lock.token_1_fees_collected = 0;
    user_lock.is_unlocked = false;
    user_lock.last_updated = block_timestamp;
    user_lock.created_at = block_timestamp;

    // update lp lock counter
    lp_lock_counter.total_lock_count = new_lock_count;
    lp_lock_counter.total_lock_amount = lp_lock_counter
    .total_lock_amount
    .checked_add(amount)
    .ok_or(ErrorCode::Overflow)?;

    emit!(
        LpLockEvent{
            user: user_lock.user,
            amount: user_lock.lock_amount,
            lp_mint: user_lock.lp_mint,
            locked_perm: lock_permanent
        }
    );

    Ok(())
}
