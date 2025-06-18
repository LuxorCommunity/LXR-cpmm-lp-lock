# Raydium CPMM LP Token Lock Contract

A Solana smart contract that enables users to lock their Raydium Constant Product Market Maker (CPMM) LP tokens for a specified duration or permanently while continuing to earn trading fees from the underlying liquidity pool.

## Overview

This contract solves a key problem in DeFi: how to lock LP tokens for governance, vesting, or commitment purposes while still allowing the locked position to accumulate trading fees. Traditional token locks would forfeit fee earnings, but this contract maintains the fee-earning capability through a sophisticated vault system and on-demand fee collection mechanism that users can trigger at any time.

## Core Features

- **Lock LP tokens** for specified durations or permanently
- **Continue earning fees** from underlying Raydium pools while tokens are locked
- **Collect accumulated fees** on-demand whenever users choose
- **Multiple locks per user** for the same LP token
- **Automatic fee calculation** based on liquidity growth
- **Time-based or permanent locks** with flexible unlock conditions

## Architecture Overview

### Core Components

#### 1. **UserLock Account (`states/lock.rs`)**
Stores individual lock information for each user position:
- **Principal tracking**: Records the original token amounts and liquidity value
- **Fee accounting**: Tracks accumulated fees from both tokens in the pair
- **Lock parameters**: Duration, permanent status, and unlock conditions
- **Unique identification**: Each lock has a unique counter to support multiple locks per user

#### 2. **LpLockCounter Account (`states/lp_lock_counter.rs`)**
Maintains aggregate statistics per user per LP token:
- **Total positions**: Counts all locks created by a user for a specific LP
- **Total amount**: Tracks cumulative LP tokens locked
- **Enables efficient querying** of user's lock portfolio

#### 3. **Vault System**
Each lock creates a dedicated token vault:
- **Unique vault per lock**: Generated using `(user, lp_mint, lock_count)` seeds
- **Authority-controlled**: Managed by the contract's PDA authority
- **Secure storage**: LP tokens remain in the vault until unlock or fee collection

### Key Mechanisms

#### Fee Earning While Locked

The contract achieves fee earning through a sophisticated calculation mechanism:

1. **Principal Liquidity Snapshot**: When tokens are locked, the contract records:
   - Original token amounts (token_0, token_1)
   - Current liquidity value (geometric mean of token amounts)
   - Total LP token amount locked

2. **Fee Calculation Process**: During fee collection:
   - Current pool state is fetched from Raydium
   - Current value of locked LP tokens is calculated
   - Principal liquidity is scaled to current LP token amount
   - Excess LP tokens represent accumulated fees
   - These excess tokens are burned to collect underlying tokens

#### Fee Collection Algorithm

```rust
// Simplified version of the fee calculation
updated_principal_lp_tokens = (principal_liquidity * lock_amount) / current_liquidity
lp_tokens_to_burn = lock_amount - updated_principal_lp_tokens
```

This ensures that:
- Principal value is preserved relative to original liquidity
- Growth in LP token value (from fees) is captured
- Users receive the fee portion as underlying tokens

## Contract Operations

### 1. Lock LP Tokens (`instructions/lock_lp.rs`)

Creates a new lock position with the following process:

**Parameters:**
- `amount`: LP tokens to lock
- `lock_duration`: Lock period in seconds (0 for permanent)
- `lock_permanent`: Boolean flag for permanent locks

**Process:**
1. Validates LP token ownership and amount
2. Creates or updates the user's lock counter
3. Initializes a new UserLock account with unique counter
4. Creates a dedicated vault for the locked tokens
5. Transfers LP tokens from user to the vault
6. Records principal liquidity and token amounts
7. Sets unlock time based on duration or permanent flag

**Key Features:**
- **Multiple locks supported**: Users can create multiple separate locks
- **Flexible timing**: Support for both time-based and permanent locks
- **Principal preservation**: Records exact liquidity value for fee calculations

### 2. Collect Fees (`instructions/collect_fees.rs`)

Allows users to claim accumulated fees at any time without unlocking principal:

**Process:**
1. Validates that the lock is still active
2. Fetches current pool state from Raydium CPMM
3. Calculates current liquidity value of the position
4. Determines excess LP tokens representing fees
5. Burns excess tokens through Raydium's withdraw function
6. Updates the lock with reduced LP amount (principal only)
7. Transfers collected tokens to user accounts

**Cross-Program Interaction:**
- Makes CPI call to Raydium CPMM program
- Uses Raydium's withdraw instruction to convert LP tokens to underlying assets
- Maintains compatibility with Raydium's fee structure

### 3. Unlock LP Tokens (`instructions/unlock_lp.rs`)

Releases locked LP tokens after the lock period expires:

**Validation:**
- Ensures lock is not permanent
- Checks that unlock time has been reached
- Verifies lock hasn't already been unlocked

**Process:**
1. Validates unlock conditions and timing
2. Marks the lock as unlocked
3. Updates aggregate counters
4. Transfers all remaining LP tokens back to user
5. Emits unlock event for tracking

## Data Structures

### UserLock Account Structure
```rust
pub struct UserLock {
    pub user: Pubkey,                    // Lock owner
    pub lp_mint: Pubkey,                 // LP token being locked
    pub lock_count: u64,                 // Unique identifier
    pub lock_amount: u64,                // Current LP tokens in lock
    pub unlock_time: u64,                // When lock expires
    pub principal_token_0: u64,          // Original token 0 amount
    pub principal_token_1: u64,          // Original token 1 amount
    pub principal_liquidity: u64,        // Original liquidity value
    pub is_locked_permanently: bool,     // Permanent lock flag
    pub token_0_fees_collected: u64,     // Accumulated fees
    pub token_1_fees_collected: u64,     // Accumulated fees
    pub is_unlocked: bool,               // Unlock status
    pub last_updated: u64,               // Last operation timestamp
    pub created_at: u64,                 // Lock creation time
}
```

### LpLockCounter Account Structure
```rust
pub struct LpLockCounter {
    pub user: Pubkey,                    // User address
    pub lp_mint: Pubkey,                 // LP token mint
    pub total_lock_count: u64,           // Number of locks created
    pub total_lock_amount: u64,          // Total LP tokens locked
}
```

## Security Features

### Access Controls
- **User-only operations**: All functions require the lock owner's signature
- **PDA authorities**: Vaults are controlled by program-derived addresses
- **Validation checks**: Extensive validation of pool states and token accounts

### State Validation
- **Pool state verification**: Ensures LP tokens match the correct Raydium pool
- **Amount validation**: Prevents zero amounts and overflow conditions
- **Time validation**: Proper handling of timestamps and duration calculations

### Error Handling
- **Comprehensive error codes**: Clear error messages for all failure conditions
- **Overflow protection**: Safe math operations throughout the contract
- **State consistency**: Atomic operations maintain consistent state

## Events and Monitoring

The contract emits events for all major operations:

### LpLockEvent
```rust
pub struct LpLockEvent {
    pub user: Pubkey,           // User who created the lock
    pub amount: u64,            // Amount of LP tokens locked
    pub lp_mint: Pubkey,        // LP token identifier
    pub locked_perm: bool,      // Whether lock is permanent
}
```

### CollectFeesEvent
```rust
pub struct CollectFeesEvent {
    pub user: Pubkey,           // User who collected fees
    pub lp_mint: Pubkey,        // LP token identifier
    pub token_0_amount: u64,    // Token 0 fees collected
    pub token_1_amount: u64,    // Token 1 fees collected
}
```

### LpUnlockEvent
```rust
pub struct LpUnlockEvent {
    pub user: Pubkey,           // User who unlocked
    pub amount: u64,            // Amount of LP tokens unlocked
    pub lp_mint: Pubkey,        // LP token identifier
}
```

## Technical Implementation

### Curve Calculator Integration
The contract integrates with Raydium's curve calculator for precise token calculations:
- Converts LP tokens to underlying asset amounts
- Maintains consistency with Raydium's pricing mechanisms
- Handles rounding and precision correctly

### Cross-Program Invocation (CPI)
- **Raydium Integration**: Makes CPI calls to Raydium CPMM for withdrawals
- **Token Operations**: Uses SPL Token program for transfers and account management
- **Authority Management**: Proper PDA signing for cross-program calls

### Account Derivation
The contract uses deterministic account derivation:
- **UserLock**: `["user_lock", user, lp_mint, lock_count]`
- **LpLockCounter**: `["lp_lock_counter", user, lp_mint]`
- **LpLockVault**: `["lp_lock_vault", user, lp_mint, lock_count]`

## Command Line Interface (CLI)

The contract includes a comprehensive CLI tool for easy interaction with all contract functions. The CLI is built using Rust and can be executed via Cargo.

### Setup and Configuration

Before using the CLI, you need to create a configuration file `client_config.ini`:

```ini
[Global]
http_url = https://api.mainnet-beta.solana.com
ws_url = wss://api.mainnet-beta.solana.com
payer_path = /path/to/your/keypair.json
admin_path = /path/to/admin/keypair.json
raydium_cp_program = CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C
slippage = 0.5
```

**Configuration Parameters:**
- `http_url`: Solana RPC endpoint for HTTP requests
- `ws_url`: Solana RPC endpoint for WebSocket connections
- `payer_path`: Path to your wallet keypair file
- `admin_path`: Path to admin keypair (if needed)
- `raydium_cp_program`: Raydium CPMM program ID
- `slippage`: Slippage tolerance for transactions

### CLI Commands

#### 1. Lock LP Tokens (Time-Based)

Lock LP tokens for a specific duration:

```bash
cargo run -p client lock-lp --pool-id <POOL_ID> --amount <AMOUNT> --duration <DURATION_SECONDS>
```

**Parameters:**
- `--pool-id`: The Raydium pool ID (Pubkey)
- `--amount`: Amount of LP tokens to lock (in token units)
- `--duration`: Lock duration in seconds

**Example:**
```bash
# Lock 100 LP tokens for 30 days (2,592,000 seconds)
cargo run -p client lock-lp \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --amount 100000000 \
  --duration 2592000
```

#### 2. Lock LP Tokens (Permanent)

Create a permanent lock that cannot be unlocked:

```bash
cargo run -p client lock-lp-permanently --pool-id <POOL_ID> --amount <AMOUNT>
```

**Parameters:**
- `--pool-id`: The Raydium pool ID (Pubkey)
- `--amount`: Amount of LP tokens to lock permanently

**Example:**
```bash
# Permanently lock 50 LP tokens
cargo run -p client lock-lp-permanently \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --amount 50000000
```

#### 3. Collect Accumulated Fees

Claim fees from a specific lock without unlocking the principal:

```bash
cargo run -p client collect-fees --pool-id <POOL_ID> --lock-id <LOCK_ID>
```

**Parameters:**
- `--pool-id`: The Raydium pool ID (Pubkey)
- `--lock-id`: The specific lock ID (u64, starts from 1)

**Example:**
```bash
# Collect fees from lock #2
cargo run -p client collect-fees \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --lock-id 2
```

#### 4. Unlock LP Tokens

Unlock LP tokens after the lock period has expired:

```bash
cargo run -p client unlock-lp --pool-id <POOL_ID> --lock-id <LOCK_ID>
```

**Parameters:**
- `--pool-id`: The Raydium pool ID (Pubkey)
- `--lock-id`: The specific lock ID (u64)

**Example:**
```bash
# Unlock lock #1 (only works if lock period has expired)
cargo run -p client unlock-lp \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --lock-id 1
```

### CLI Workflow Examples

#### Example 1: Basic Lock and Fee Collection
```bash
# 1. Lock 100 LP tokens for 7 days
cargo run -p client lock-lp \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --amount 100000000 \
  --duration 604800

# 2. Wait for some time for fees to accumulate...

# 3. Collect fees from the first lock (lock-id 1)
cargo run -p client collect-fees \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --lock-id 1

# 4. After 7 days, unlock the tokens
cargo run -p client unlock-lp \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --lock-id 1
```

#### Example 2: Multiple Locks Strategy
```bash
# Create multiple locks with different strategies
# Short-term lock
cargo run -p client lock-lp \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --amount 50000000 \
  --duration 2592000  # 30 days

# Medium-term lock  
cargo run -p client lock-lp \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --amount 75000000 \
  --duration 7776000  # 90 days

# Permanent commitment
cargo run -p client lock-lp-permanently \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --amount 25000000

# Collect fees from any lock as they accumulate
cargo run -p client collect-fees \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --lock-id 1

cargo run -p client collect-fees \
  --pool-id 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --lock-id 2
```

### Important Notes

#### Lock ID Management
- Lock IDs are automatically assigned starting from 1
- Each user's locks are numbered sequentially per LP token
- Lock IDs are required for `collect-fees` and `unlock-lp` operations
- You can track your lock IDs by monitoring transaction logs or using blockchain explorers

#### Transaction Confirmation
- All CLI commands return a transaction signature upon successful execution
- Use Solana blockchain explorers to verify transaction status
- Failed transactions will display error messages with specific error codes

#### Token Amounts
- LP token amounts should be specified in the token's base units
- Check the LP token's decimal places to calculate the correct amount
- Most LP tokens use 6-9 decimal places

#### Prerequisites
- Ensure you have sufficient SOL for transaction fees
- Your wallet must own the LP tokens you want to lock
- LP tokens must be from supported Raydium CPMM pools
- For unlocking, the lock period must have expired (except for permanent locks)

### Troubleshooting

**Common Issues:**
- **"Insufficient balance"**: Ensure you have enough LP tokens and SOL for fees
- **"Unlock time not reached"**: Wait until the lock period expires before unlocking
- **"Lock already unlocked"**: This lock has already been unlocked
- **"Lock is permanent"**: Permanent locks cannot be unlocked

**Debugging:**
- Use `--help` flag with any command to see parameter details
- Check your `client_config.ini` file for correct RPC endpoints and paths
- Verify pool IDs are correct for the intended Raydium pools
- Monitor Solana network status for potential RPC issues

## Usage Considerations

### Gas Optimization
- **Minimal account creation**: Only creates necessary accounts
- **Efficient calculations**: Optimized mathematical operations
- **Batch operations**: Collect fees for multiple positions efficiently

### Integration Points
- **Frontend Integration**: Events enable real-time UI updates
- **Analytics**: Comprehensive tracking of lock metrics
- **Portfolio Management**: Support for multiple concurrent locks

### Risk Considerations
- **Smart contract risk**: Standard smart contract audit recommendations apply
- **Pool dependency**: Relies on Raydium pool state consistency
- **Fee calculation accuracy**: Dependent on accurate liquidity measurements

## Future Enhancements

Potential areas for expansion:
- **Multi-token fee collection**: Batch collection across multiple locks
- **Advanced lock types**: Support for vesting schedules or conditional unlocks
- **Governance integration**: Lock-based voting weight calculations
- **Cross-pool strategies**: Support for multiple pool types

This contract provides a robust foundation for LP token locking while maintaining fee earning capabilities, essential for modern DeFi applications requiring both liquidity commitment and yield generation.

---
### Special Thanks
This smart contract was developed with the incredible support of [Vishesh Sachdev](https://github.com/vishesh0123).  
Reliable, secure, and efficient — exactly what the Luxor ecosystem needed.
