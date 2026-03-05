# 🌊 ApexFlow AMM

**ApexFlow** is a high-performance, non-custodial Automated Market Maker (AMM) built on the Solana blockchain using the Anchor framework. It implements a constant-product invariant ($x \cdot y = k$) with configurable fee tiers, protocol fee collection, and liquidity provider tokens.

**[View Deployment on Solana Explorer](https://explorer.solana.com/tx/5CVN7zWGcPiopVXtS6d31zKZGXHX4BrZdHeychjSUb7sG8JU4yavMEFQmqsU1awfSg5znK4GqkXiDFor1ds4mhh?cluster=devnet)**

---

## 🚀 Key Features

* **Permissionless Pool Creation**: Launch liquidity pools for any SPL token pair.
* **Flexible Fee Tiers**: Four distinct tiers (0.1%, 0.25%, 1%, and 4%) to cater to different asset volatilities.
* **LP Token Dynamics**: Automated minting/burning of LP tokens proportional to pool share.
* **Admin Controls**: Global "Circuit Breaker" to pause or unpause pools in emergencies.

---

## 🏗️ Architecture & Account Model

ApexFlow uses a Program Derived Address (PDA) architecture to ensure security and canonical state management. The diagram below illustrates the relationship between the global state, individual pools, and user wallets.

![ApexFlow Account Model](images/flow2.png)

### Core Account Definitions

| Account | Seeds | Purpose |
| :--- | :--- | :--- |
| **OperationState** | `["operation_state"]` | Singleton PDA storing the global admin key. |
| **AmmConfig** | `["amm_config", index]` | Stores fee rates for a specific tier. |
| **PoolState** | `["pool", mint_a, mint_b, config]` | Central state tracking vaults, mints, and fees. |
| **LpMint** | `["lp_mint", pool_state]` | Mint account for LP tokens, authorized by the pool. |

---

## 🛡️ Security Architecture

The protocol implements multiple layers of protection to ensure the safety of user funds and the integrity of the liquidity pools:

* **Context Validation**: Every instruction performs a `check_context` to verify the program ID and ensure no unexpected accounts are injected, preventing account substitution attacks.
* **Canonical Pool Enforcement**: By requiring `mint_a < mint_b` (ordered by public key), the program prevents the creation of duplicate or fragmented liquidity pools for the same pair.
* **Integer Safety & Overflow Protection**: All mathematical operations utilize `u128` intermediate values. Any calculation that would result in an overflow returns a specific `MathOverflow` error rather than panicking.
* **Minimum Liquidity Lock**: To protect against "Pool Draining" or rounding-based price manipulation, 100 LP tokens are permanently locked to the system program address upon the first deposit.
* **Emergency Circuit Breaker**: The global admin can use `update_pool_status` to pause all swaps, deposits, and withdrawals if a vulnerability or market anomaly is detected.

---

## 📊 Mathematical Logic

### Swap Invariant

Fees are deducted from the input amount before calculating the output to protect the liquidity providers:

$$
trade\_fee = amount\_in \times \frac{trade\_fee\_rate}{10^9}
$$

$$
effective\_in = amount\_in - trade\_fee
$$

$$
amount\_out = \frac{reserve\_out \times effective\_in}{reserve\_in + effective\_in}
$$

### Fee Distribution

The protocol collects a portion of every trade fee for ecosystem maintenance:

* **Protocol Cut**: 12% of the trade fee.
* **Fund Cut**: 4% of the trade fee.

---

## 🛠️ Developer Guide

### Prerequisites

* [Rust](https://rustup.rs/) & [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
* [Anchor Framework](https://www.anchor-lang.com/docs/installation)

### Build & Test

```bash
# Clone the repository
git clone [https://github.com/s1mpleog/apex-flow](https://github.com/s1mpleog/apex-flow)

# Build the program
anchor build

# Run local tests with testing features enabled
anchor build -- --features local-testing && anchor test --skip-build
