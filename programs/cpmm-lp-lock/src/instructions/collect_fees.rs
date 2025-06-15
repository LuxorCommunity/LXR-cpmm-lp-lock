use crate::curve::CurveCalculator;
use crate::curve::RoundDirection;
use crate::error::ErrorCode;
use crate::states::*;
use crate::utils::transfer_from_pool_vault_to_user;
use crate::utils::U128;
use crate::LP_LOCK_VAULT_SEED;
use anchor_lang::prelude::borsh::BorshDeserialize;
use anchor_lang::prelude::borsh::BorshSerialize;
use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_2022::Token2022;
use anchor_spl::memo::spl_memo;
use anchor_spl::token_interface::{Mint, TokenAccount};
use anchor_lang::{solana_program::clock};
use anchor_lang::{solana_program::{instruction::Instruction, program::{invoke}}};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Withdraw {
    lp_token_amount: u64,
    minimum_token_0_amount: u64,
    minimum_token_1_amount: u64,
}

#[derive(Accounts)]
pub struct CollectFees<'info> {
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
        mut,
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

    /// Lp token mint
    #[account(
        mut,
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
        constraint = user_lp_lock.lp_mint == lp_mint.key(),
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

    /// The owner's token account for receive token_0
    #[account(
        mut,
        token::mint = token_0_vault.mint,
        token::authority = owner
    )]
    pub token_0_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The owner's token account for receive token_1
    #[account(
        mut,
        token::mint = token_1_vault.mint,
        token::authority = owner
    )]
    pub token_1_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The address that holds pool tokens for token_0
    #[account(mut)]
    pub token_0_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The address that holds pool tokens for token_1
    #[account(mut)]
    pub token_1_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// token Program
    pub token_program: Program<'info, Token>,

    /// Token program 2022
    pub token_program_2022: Program<'info, Token2022>,

    /// The mint of token_0 vault
    #[account(
        address = token_0_vault.mint
    )]
    pub vault_0_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The mint of token_1 vault
    #[account(
        address = token_1_vault.mint
    )]
    pub vault_1_mint: Box<InterfaceAccount<'info, Mint>>,

    /// memo program
    /// CHECK:
    #[account(
        address = spl_memo::id()
    )]
    pub memo_program: UncheckedAccount<'info>,

    /// CHECK: This account is owned by another program
    #[account(
        mut,
        address = crate::raydium_cpmm::id()
    )]
    pub raydium_cpmm_program: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn collect_fees(ctx: Context<CollectFees>) -> Result<()> {
    let user_lock = &mut ctx.accounts.user_lp_lock;
    let lp_lock_counter = &mut ctx.accounts.lp_lock_counter;

    require_eq!(
        user_lock.is_unlocked,
        false,
        ErrorCode::LockAlreadyUnlocked
    );

    let pool_state_info = &ctx.accounts.pool_state;
    let pool_state = PoolState::try_deserialize(&mut &pool_state_info.data.borrow()[..])?;
    require_eq!(pool_state.lp_mint,ctx.accounts.lp_mint.key(), ErrorCode::IncorrectLpMint);
    require_eq!(pool_state.token_0_vault,ctx.accounts.token_0_vault.key());
    require_eq!(pool_state.token_1_vault,ctx.accounts.token_1_vault.key());

    let (total_token_0_amount, total_token_1_amount) = pool_state.vault_amount_without_fee(
        ctx.accounts.token_0_vault.amount,
        ctx.accounts.token_1_vault.amount,
    );

    msg!("Total token 0 amount in the pool: {}", total_token_0_amount);
    msg!("Total token 1 amount in the pool: {}", total_token_1_amount);

    let results = CurveCalculator::lp_tokens_to_trading_tokens(
        u128::from(user_lock.lock_amount),
        u128::from(pool_state.lp_supply),
        u128::from(total_token_0_amount),
        u128::from(total_token_1_amount),
        RoundDirection::Floor,
    )
    .ok_or(ErrorCode::ZeroTradingTokens)?;

    msg!("Locked LP amount: {}", user_lock.lock_amount);
    msg!("Token 0 amount belongs to locked LP: {}", results.token_0_amount);
    msg!("Token 1 amount belongs to locked LP: {}", results.token_1_amount);

    let liquidity = U128::from(results.token_0_amount)
        .checked_mul(results.token_1_amount.into())
        .unwrap()
        .integer_sqrt()
        .as_u64();

    msg!("Liquidity belongs to locked LP: {}", liquidity);

    require_gt!(liquidity, 0, ErrorCode::ZeroLiquidity);

    msg!("Principal liquidity : {}", user_lock.principal_liquidity);
    let updated_principal_lp_tokens = U128::from(user_lock.principal_liquidity)
        .checked_mul(user_lock.lock_amount.into())
        .unwrap()
        .checked_div(liquidity.into())
        .unwrap().as_u64();
    msg!("Updated principal LP tokens: {}", updated_principal_lp_tokens);
    let lp_tokens_to_burn = user_lock.lock_amount
        .checked_sub(updated_principal_lp_tokens)
        .ok_or(ErrorCode::Overflow)?;
    msg!("LP tokens to burn: {}", lp_tokens_to_burn);
    require_gt!(lp_tokens_to_burn, 0, ErrorCode::ZeroLpTokensToBurn);

    let results = CurveCalculator::lp_tokens_to_trading_tokens(
        u128::from(lp_tokens_to_burn),
        u128::from(pool_state.lp_supply),
        u128::from(total_token_0_amount),
        u128::from(total_token_1_amount),
        RoundDirection::Floor,
    )
    .ok_or(ErrorCode::ZeroTradingTokens)?;

    msg!("Token 0 amount belongs to lp to burn: {}", results.token_0_amount);
    msg!("Token 1 amount belongs to lp to burn {}", results.token_1_amount);

    let token_0_amount = u64::try_from(results.token_0_amount).unwrap();
    let token_0_amount = std::cmp::min(total_token_0_amount, token_0_amount);

    let token_1_amount = u64::try_from(results.token_1_amount).unwrap();
    let token_1_amount = std::cmp::min(total_token_1_amount, token_1_amount);

    msg!("Final token 0 amount to receive: {}", token_0_amount);
    msg!("Final token 1 amount to receive: {}", token_1_amount);

    require!(
        token_0_amount > 0 && token_1_amount > 0,
        ErrorCode::ZeroTradingTokens
    );

    lp_lock_counter.total_lock_amount = lp_lock_counter
    .total_lock_amount
    .checked_sub(user_lock.lock_amount)
    .ok_or(ErrorCode::UnderflowError)?;

    // update user lock
    user_lock.lock_amount = updated_principal_lp_tokens;
    
    lp_lock_counter.total_lock_amount = lp_lock_counter
    .total_lock_amount
    .checked_add(user_lock.lock_amount)
    .ok_or(ErrorCode::Overflow)?;

    user_lock.token_0_fees_collected = user_lock
        .token_0_fees_collected
        .checked_add(token_0_amount)
        .ok_or(ErrorCode::Overflow)?;
    user_lock.token_1_fees_collected = user_lock
        .token_1_fees_collected
        .checked_add(token_1_amount)
        .ok_or(ErrorCode::Overflow)?;
    user_lock.last_updated = clock::Clock::get()?.unix_timestamp.try_into().unwrap();

    transfer_from_pool_vault_to_user(
        ctx.accounts.lock_vault_authority.to_account_info(),
        ctx.accounts.lp_lock_vault.to_account_info(),
        ctx.accounts.owner_lp_token.to_account_info(),
        ctx.accounts.lp_mint.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        lp_tokens_to_burn,
        ctx.accounts.lp_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[ctx.bumps.lock_vault_authority]]],
    )?;

    let params = Withdraw {
        lp_token_amount: lp_tokens_to_burn,
        minimum_token_0_amount: 0,
        minimum_token_1_amount: 0,
    };

    let discriminator =
        anchor_lang::solana_program::hash::hash(b"global:withdraw").to_bytes()[..8].to_vec();
    let mut data = discriminator;
    data.extend(params.try_to_vec()?);

    let authority = ctx.accounts.authority.key();
    let pool_state = ctx.accounts.pool_state.key();
    let owner_lp_token = ctx.accounts.owner_lp_token.key();
    let token_0_account = ctx.accounts.token_0_account.key();
    let token_1_account = ctx.accounts.token_1_account.key();
    let token_0_vault = ctx.accounts.token_0_vault.key();
    let token_1_vault = ctx.accounts.token_1_vault.key();
    let token_program = ctx.accounts.token_program.key();
    let token_program_2022 = ctx.accounts.token_program_2022.key();
    let vault_0_mint = ctx.accounts.vault_0_mint.key();
    let vault_1_mint = ctx.accounts.vault_1_mint.key();
    let lp_mint = ctx.accounts.lp_mint.key();
    let memo_program = ctx.accounts.memo_program.key();

    let accounts = vec![
        AccountMeta::new(user_lock.user, true),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new(pool_state, false),
        AccountMeta::new(owner_lp_token, false),
        AccountMeta::new(token_0_account, false),
        AccountMeta::new(token_1_account, false),
        AccountMeta::new(token_0_vault, false),
        AccountMeta::new(token_1_vault, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(token_program_2022, false),
        AccountMeta::new_readonly(vault_0_mint, false),
        AccountMeta::new_readonly(vault_1_mint, false),
        AccountMeta::new(lp_mint, false),
        AccountMeta::new_readonly(memo_program, false),
    ];

    let ix = Instruction {
        program_id: crate::raydium_cpmm::id(),
        accounts,
        data 
    };

    let accounts = Box::new(vec![
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.pool_state.to_account_info(),
        ctx.accounts.owner_lp_token.to_account_info(),
        ctx.accounts.token_0_account.to_account_info(),
        ctx.accounts.token_1_account.to_account_info(),
        ctx.accounts.token_0_vault.to_account_info(),
        ctx.accounts.token_1_vault.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.token_program_2022.to_account_info(),
        ctx.accounts.vault_0_mint.to_account_info(),
        ctx.accounts.vault_1_mint.to_account_info(),
        ctx.accounts.lp_mint.to_account_info(),
        ctx.accounts.memo_program.to_account_info()
    ]);

    invoke(&ix, &*accounts)?;

    emit!(
        CollectFeesEvent {
            user: ctx.accounts.owner.key(),
            lp_mint: ctx.accounts.lp_mint.key(),
            token_0_amount,
            token_1_amount,
        }
    );


    Ok(())
}
