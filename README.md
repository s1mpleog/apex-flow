```markdown
# ApexFlow — Constant Product AMM

A Solana AMM protocol built with Anchor, implementing the constant 
product invariant (x*y=k) with multiple fee tiers and production-grade
account architecture.

> Program ID: `DshyYCvcP7cj1BUhH3VqwfYG8iPie5XzJPB8UGuUsdJD`
> Status: In development — tests in progress, devnet deployment coming soon

## Architecture

![Account Flow](images/flow2.png)

ApexFlow follows a layered account hierarchy:

- **OperationState** — global admin PDA, controls protocol-level permissions
- **AmmConfig** — per-fee-tier configuration PDA, seeded by fee index
- **PoolState** — unique per token pair + fee tier, owns all pool assets
- **Vaults** — SPL token accounts owned by PoolState PDA
- **LP Mint** — unique per pool, authority is PoolState PDA

This means the same token pair (e.g. USDC/WSOL) can have multiple 
pools at different fee tiers simultaneously — exactly like Raydium CPMM.

## Fee Tiers

| Index | Trade Fee |
|-------|-----------|
| 0     | 0.1%      |
| 1     | 0.25%     |
| 2     | 1%        |
| 3     | 4%        |

Each trade fee is split into three components:
- **LP fee** — stays in vault, accrues to liquidity providers silently
- **Protocol fee** — 12% of trade fee, collectable by admin
- **Fund fee** — 4% of trade fee, collectable by admin

## Swap Math

ApexFlow uses the constant product formula:

```
amount_out = (y * dx) / (x + dx)
```

Where `dx` is `amount_in` after fee deduction. All intermediate 
calculations use `u128` to prevent overflow when multiplying two `u64` 
reserve values.

## Security

- Vault identity verified against PoolState on every instruction
- Mint ordering enforced (`mint_a < mint_b`) to prevent duplicate pools
- Minimum liquidity permanently locked to dead address on first deposit
- Admin authority verified through OperationState PDA
- `check_context` on every instruction rejects unexpected accounts
- Pool status flag allows emergency pause

## Instructions

| Instruction | Description |
|-------------|-------------|
| `initialize_operation_state` | Bootstrap global admin state |
| `initialize_amm_config` | Create fee tier configuration |
| `initialize_pool` | Create pool for token pair + fee tier |
| `deposit` | Add liquidity, receive LP tokens |
| `withdraw` | Burn LP tokens, receive underlying tokens |
| `swap` | Swap token A for B or B for A |
| `collect_protocol_fee` | Admin collects protocol fees |
| `collect_fund_fee` | Admin collects fund fees |
| `update_pool_status` | Admin pause/unpause pool |

## Getting Started

```bash
# Install dependencies
yarn install

# Build
anchor build

# Test
anchor test

# Deploy to devnet
anchor deploy --provider.cluster devnet
```

## Built With

- [Anchor](https://anchor-lang.com) — Solana framework
- [SPL Token](https://spl.solana.com/token) — Token program
- Rust

## License

MIT
```

---
