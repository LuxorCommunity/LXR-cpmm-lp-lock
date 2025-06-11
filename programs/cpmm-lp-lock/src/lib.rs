pub mod curve;
pub mod error;
pub mod instructions;
pub mod states;
pub mod utils;
use anchor_lang::prelude::*;
use instructions::*;

#[cfg(not(feature = "no-entrypoint"))]
solana_security_txt::security_txt! {
    name: "cpmm_lp_lock"
}

#[cfg(feature = "devnet")]
declare_id!("FFNVCqrn1yPZrfDsQ4eN2ctbSWFkuJo95FnXrVFeh2Et");
#[cfg(not(feature = "devnet"))]
declare_id!("FFNVCqrn1yPZrfDsQ4eN2ctbSWFkuJo95FnXrVFeh2Et");

pub mod raydium_cpmm {
    use anchor_lang::prelude::declare_id;
    #[cfg(feature = "devnet")]
    declare_id!("CPMDWBwJDtYax9qW7AyRuVC19Cc4L4Vcy4n2BHAbHkCW");
    #[cfg(not(feature = "devnet"))]
    declare_id!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
}

pub mod vault_and_lp_mint_auth {
    use anchor_lang::prelude::declare_id;
    #[cfg(feature = "devnet")]
    declare_id!("7rQ1QFNosMkUCuh7Z7fPbTHvh73b68sQYdirycEzJVuw");
    #[cfg(not(feature = "devnet"))]
    declare_id!("3f7GcQFG397GAaEnv51zR6tsTVihYRydnydDD1cXekxH");
}

pub const AUTH_SEED: &str = "lock_lp_auth_seed";
pub const LP_LOCK_VAULT_SEED: &str = "lock_lp_vault";

#[program]
pub mod solar_cp_swap {
    use super::*;

    pub fn lock_lp(ctx: Context<LockLp>, amount: u64, lock_duration: u64) -> Result<()> {
        instructions::lock_lp(
            ctx,
            amount,
            if lock_duration == 0 { 1 } else { lock_duration },
            false,
        )
    }

    pub fn lock_lp_permanent(ctx: Context<LockLp>, amount: u64) -> Result<()> {
        instructions::lock_lp(ctx, amount, 0, true)
    }

    pub fn unlock_lp(ctx: Context<UnlockLp>) -> Result<()> {
        instructions::unlock_lp(ctx)
    }

    pub fn collect_fees(ctx: Context<CollectFees>) -> Result<()> {
        instructions::collect_fees(ctx)
    }
}
