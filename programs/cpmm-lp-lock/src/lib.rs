use anchor_lang::prelude::*;

#[cfg(feature = "devnet")]
declare_id!("6BtsN2jo6ijrwCUnMPzXcB8G4hYpASgpHaKWQmHbubV9");

#[cfg(not(feature = "devnet"))]
declare_id!("6BtsN2jo6ijrwCUnMPzXcB8G4hYpASgpHaKWQmHbubV9");

#[cfg(feature = "devnet")]
pub mod raydium_cpmm {
    use anchor_lang::prelude::declare_id;
    declare_id!("CPMDWBwJDtYax9qW7AyRuVC19Cc4L4Vcy4n2BHAbHkCW");
}

#[cfg(not(feature = "devnet"))]
pub mod raydium_cpmm {
    use anchor_lang::prelude::declare_id;
    declare_id!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
}

#[cfg(feature = "devnet")]
pub mod vault_and_lp_mint_auth {
    use anchor_lang::prelude::declare_id;
    declare_id!("7rQ1QFNosMkUCuh7Z7fPbTHvh73b68sQYdirycEzJVuw");
}

#[cfg(not(feature = "devnet"))]
pub mod vault_and_lp_mint_auth {
    use anchor_lang::prelude::declare_id;
    declare_id!("GpMZbSM2GgvTKHJirzeGfMFoaZ8UR2X7F4v8vHTvxFbL");
}

pub const AUTH_SEED: &str = "lock_lp_auth_seed";
pub const LP_LOCK_VAULT_SEED: &str = "lock_lp_vault";

#[cfg(not(feature = "no-entrypoint"))]
solana_security_txt::security_txt! {
    name: "cpmm-lp-lock",
    project_url: "",
    contacts: "",
    policy: "",
    source_code: "",
    preferred_languages: "en",
    auditors: ""
}

pub mod curve;
pub mod error;
pub mod instructions;
pub mod states;
pub mod utils;

use instructions::*;

#[program]
pub mod cpmm_lp_lock {
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
