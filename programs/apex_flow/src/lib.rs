use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use crate::error::ApexFlowError;

declare_id!("DshyYCvcP7cj1BUhH3VqwfYG8iPie5XzJPB8UGuUsdJD");

pub mod error;

pub const SUPPORTED_FEE_TIERS: [u64; 4] = [
    1_000_000,  // 0.1%
    2_500_000,  // 0.25%
    10_000_000, // 1%
    40_000_000, // 4%
];

const FEE_RATE_DENOMINATOR: u128 = 1_000_000_000;
const MINIMUM_LIQUIDITY: u64 = 100;

fn check_context<T>(ctx: &Context<T>) -> Result<()>
where
    T: anchor_lang::Bumps,
{
    if !check_id(ctx.program_id) {
        return err!(ApexFlowError::InvalidProgramId);
    }
    if !ctx.remaining_accounts.is_empty() {
        return err!(ApexFlowError::UnexpectedAccount);
    }
    Ok(())
}

pub fn sqrt(n: u128) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x as u64
}

pub fn to_u64(n: u128) -> Result<u64> {
    let result = u64::try_from(n).map_err(|_| ApexFlowError::MathOverflow)?;
    Ok(result)
}

#[program]
pub mod apex_flow {

    use anchor_spl::token::{burn, mint_to, transfer_checked, Burn, MintTo, TransferChecked};

    use super::*;

    pub fn initialize_operation_state(ctx: Context<InitializeOperationState>) -> Result<()> {
        check_context(&ctx)?;
        ctx.accounts.operation_state.admin_key = ctx.accounts.signer.key();
        ctx.accounts.operation_state.bump = ctx.bumps.operation_state;

        Ok(())
    }

    pub fn initialize_amm_config(
        ctx: Context<InitializeAmmConfig>,
        trade_fee_rate: u64,
        fee_index: u8,
    ) -> Result<()> {
        check_context(&ctx)?;
        require!(fee_index <= 3, ApexFlowError::InvalidFeeRate);

        require!(
            ctx.accounts.operation_state.admin_key == ctx.accounts.admin.key(),
            ApexFlowError::UnauthorizedAdmin
        );

        require!(
            SUPPORTED_FEE_TIERS.contains(&trade_fee_rate),
            ApexFlowError::InvalidFeeRate
        );

        ctx.accounts.amm_config.admin_key = ctx.accounts.admin.key();
        ctx.accounts.amm_config.fee_index = fee_index;
        ctx.accounts.amm_config.trade_fee_rate = trade_fee_rate;
        ctx.accounts.amm_config.protocol_fee_rate = 120_000_000;
        ctx.accounts.amm_config.fund_fee_rate = 40_000_000;
        ctx.accounts.amm_config.bump = ctx.bumps.amm_config;

        Ok(())
    }

    pub fn initialize_pool(ctx: Context<InitializePool>) -> Result<()> {
        check_context(&ctx)?;
        require!(
            ctx.accounts.mint_a.key() < ctx.accounts.mint_b.key(),
            ApexFlowError::InvalidMintOrder
        );

        ctx.accounts.pool_state.mint_a = ctx.accounts.mint_a.key();
        ctx.accounts.pool_state.mint_b = ctx.accounts.mint_b.key();
        ctx.accounts.pool_state.vault_a = ctx.accounts.vault_a.key();
        ctx.accounts.pool_state.vault_b = ctx.accounts.vault_b.key();
        ctx.accounts.pool_state.lp_mint = ctx.accounts.lp_mint.key();
        ctx.accounts.pool_state.amm_config = ctx.accounts.amm_config.key();
        ctx.accounts.pool_state.lp_mint_bump = ctx.bumps.lp_mint;
        ctx.accounts.pool_state.pool_bump = ctx.bumps.pool_state;
        ctx.accounts.pool_state.status = true;

        Ok(())
    }

    pub fn deposit(
        ctx: Context<Deposit>,
        lp_amount: u64,
        max_amount_a: u64,
        max_amount_b: u64,
    ) -> Result<()> {
        check_context(&ctx)?;

        require!(
            ctx.accounts.pool_state.status,
            ApexFlowError::InvalidPoolStatus
        );

        let amount_a: u128;
        let amount_b: u128;
        let lp_tokens: u128;

        let mint_a_key = ctx.accounts.mint_a.key();
        let mint_b_key = ctx.accounts.mint_b.key();
        let amm_cfg_key = ctx.accounts.amm_config.key();

        let seeds = [
            b"pool_state",
            mint_a_key.as_ref(),
            mint_b_key.as_ref(),
            amm_cfg_key.as_ref(),
            &[ctx.accounts.pool_state.pool_bump],
        ];

        let signer_seeds = &[&seeds[..]];

        // first deposit ?
        if ctx.accounts.vault_a.amount == 0 && ctx.accounts.vault_b.amount == 0 {
            amount_a = max_amount_a as u128;
            amount_b = max_amount_b as u128;

            let total_amount = amount_a * amount_b;

            require!(
                sqrt(total_amount) > MINIMUM_LIQUIDITY,
                ApexFlowError::InsufficientInitialLiquidity
            );

            lp_tokens = (sqrt(total_amount) - MINIMUM_LIQUIDITY) as u128;

            let cpi_context = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    authority: ctx.accounts.pool_state.to_account_info(),
                    mint: ctx.accounts.lp_mint.to_account_info(),
                    to: ctx.accounts.dead_address.to_account_info(),
                },
                signer_seeds,
            );

            mint_to(cpi_context, MINIMUM_LIQUIDITY)?;
        } else {
            // calculate the amount a and b
            let total_supply = ctx.accounts.lp_mint.supply;

            amount_a =
                (ctx.accounts.vault_a.amount as u128 * lp_amount as u128) / total_supply as u128;

            amount_b =
                (ctx.accounts.vault_b.amount as u128 * lp_amount as u128) / total_supply as u128;

            let amount_a_as_u64 = to_u64(amount_a)?;
            let amount_b_as_u64 = to_u64(amount_b)?;

            require!(
                amount_a_as_u64 <= max_amount_a,
                ApexFlowError::ExceededMaximumInputAmount
            );

            require!(
                amount_b_as_u64 <= max_amount_b,
                ApexFlowError::ExceededMaximumInputAmount
            );

            lp_tokens = lp_amount as u128;
        }

        let amount_a_as_u64 = to_u64(amount_a)?;
        let amount_b_as_u64 = to_u64(amount_b)?;
        let lp_tokens_as_u64 = to_u64(lp_tokens)?;

        // transfer funds

        // 1. from user_token_account_a to vault_a
        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_token_account_a.to_account_info(),
                to: ctx.accounts.vault_a.to_account_info(),
                mint: ctx.accounts.mint_a.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );

        transfer_checked(cpi_context, amount_a_as_u64, ctx.accounts.mint_a.decimals)?;

        // 2. from user_token_account_b to vault_b
        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_token_account_b.to_account_info(),
                to: ctx.accounts.vault_b.to_account_info(),
                mint: ctx.accounts.mint_b.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );

        transfer_checked(cpi_context, amount_b_as_u64, ctx.accounts.mint_b.decimals)?;

        // 3. mint tokens to user user_lp_mint_account
        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                authority: ctx.accounts.pool_state.to_account_info(),
                mint: ctx.accounts.lp_mint.to_account_info(),
                to: ctx.accounts.user_lp_mint_account.to_account_info(),
            },
            signer_seeds,
        );

        mint_to(cpi_context, lp_tokens_as_u64)?;

        Ok(())
    }

    pub fn withdraw(
        ctx: Context<Withdraw>,
        lp_tokens_to_burn: u64,
        min_amount_a: u64,
        min_amount_b: u64,
    ) -> Result<()> {
        check_context(&ctx)?;

        require!(
            ctx.accounts.pool_state.status,
            ApexFlowError::InvalidPoolStatus
        );

        require!(
            lp_tokens_to_burn > 0,
            ApexFlowError::BelowMinimumWithdrawAmount
        );

        let total_supply = ctx.accounts.lp_mint.supply;

        // burn the lp tokens user_lp_mint_account

        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.lp_mint.to_account_info(),
                from: ctx.accounts.user_lp_mint_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );

        burn(cpi_context, lp_tokens_to_burn)?;

        let amount_a = (ctx.accounts.vault_a.amount as u128 * lp_tokens_to_burn as u128)
            / total_supply as u128;

        let amount_b = (ctx.accounts.vault_b.amount as u128 * lp_tokens_to_burn as u128)
            / total_supply as u128;

        let amount_a_u64 = to_u64(amount_a)?;
        let amount_b_u64 = to_u64(amount_b)?;

        require!(
            amount_a_u64 >= min_amount_a,
            ApexFlowError::BelowMinimumWithdrawAmount
        );

        require!(
            amount_b_u64 >= min_amount_b,
            ApexFlowError::BelowMinimumWithdrawAmount
        );

        let mint_a_key = ctx.accounts.mint_a.key();
        let mint_b_key = ctx.accounts.mint_b.key();
        let amm_cfg_key = ctx.accounts.amm_config.key();

        let seeds = [
            b"pool_state",
            mint_a_key.as_ref(),
            mint_b_key.as_ref(),
            amm_cfg_key.as_ref(),
            &[ctx.accounts.pool_state.pool_bump],
        ];

        let signer_seeds = &[&seeds[..]];

        // transfer vault_a to user_token_account_a
        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.vault_a.to_account_info(),
                to: ctx.accounts.user_token_account_a.to_account_info(),
                mint: ctx.accounts.mint_a.to_account_info(),
                authority: ctx.accounts.pool_state.to_account_info(),
            },
            signer_seeds,
        );

        transfer_checked(cpi_context, amount_a_u64, ctx.accounts.mint_a.decimals)?;

        // transfer vault_b to user_token_account_b
        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.vault_b.to_account_info(),
                to: ctx.accounts.user_token_account_b.to_account_info(),
                mint: ctx.accounts.mint_b.to_account_info(),
                authority: ctx.accounts.pool_state.to_account_info(),
            },
            signer_seeds,
        );

        transfer_checked(cpi_context, amount_b_u64, ctx.accounts.mint_b.decimals)?;

        Ok(())
    }

    pub fn swap(ctx: Context<Swap>, amount_in: u64, min_amount_out: u64) -> Result<()> {
        check_context(&ctx)?;
        require!(
            ctx.accounts.pool_state.status,
            ApexFlowError::InvalidPoolStatus
        );
        require!(amount_in > 0, ApexFlowError::ExceededMaximumInputAmount);

        let trade_fee = (amount_in as u128 * ctx.accounts.amm_config.trade_fee_rate as u128)
            / FEE_RATE_DENOMINATOR;
        let trade_fee_u64 = to_u64(trade_fee)?;
        let amount_in_effective = amount_in - trade_fee_u64;

        let protocol_fee =
            (trade_fee * ctx.accounts.amm_config.protocol_fee_rate as u128) / FEE_RATE_DENOMINATOR;
        let fund_fee =
            (trade_fee * ctx.accounts.amm_config.fund_fee_rate as u128) / FEE_RATE_DENOMINATOR;

        let amount_out = (ctx.accounts.vault_b.amount as u128 * amount_in_effective as u128)
            / (ctx.accounts.vault_a.amount as u128 + amount_in_effective as u128);

        let amount_out_u64 = to_u64(amount_out)?;
        let protocol_fee_u64 = to_u64(protocol_fee)?;
        let fund_fee_u64 = to_u64(fund_fee)?;

        require!(
            amount_out_u64 >= min_amount_out,
            ApexFlowError::ExceededSlippageTolerance
        );

        ctx.accounts.pool_state.protocol_fees_a += protocol_fee_u64;
        ctx.accounts.pool_state.fund_fees_a += fund_fee_u64;

        let mint_a_key = ctx.accounts.mint_a.key();
        let mint_b_key = ctx.accounts.mint_b.key();
        let amm_cfg_key = ctx.accounts.amm_config.key();

        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.input_token_account.to_account_info(),
                to: ctx.accounts.vault_a.to_account_info(),
                mint: ctx.accounts.mint_a.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        transfer_checked(cpi_context, amount_in, ctx.accounts.mint_a.decimals)?;

        let seeds = [
            b"pool_state",
            mint_a_key.as_ref(),
            mint_b_key.as_ref(),
            amm_cfg_key.as_ref(),
            &[ctx.accounts.pool_state.pool_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.vault_b.to_account_info(),
                to: ctx.accounts.output_token_account.to_account_info(),
                mint: ctx.accounts.mint_b.to_account_info(),
                authority: ctx.accounts.pool_state.to_account_info(),
            },
            signer_seeds,
        );
        transfer_checked(cpi_context, amount_out_u64, ctx.accounts.mint_b.decimals)?;

        Ok(())
    }

    // admin instructions
    pub fn collect_protocol_fee(ctx: Context<CollectProtocolFee>) -> Result<()> {
        check_context(&ctx)?;

        let mint_a_key = ctx.accounts.mint_a.key();
        let mint_b_key = ctx.accounts.mint_b.key();
        let amm_cfg_key = ctx.accounts.amm_config.key();

        let seeds = [
            b"pool_state",
            mint_a_key.as_ref(),
            mint_b_key.as_ref(),
            amm_cfg_key.as_ref(),
            &[ctx.accounts.pool_state.pool_bump],
        ];

        let signer_seeds = &[&seeds[..]];

        if ctx.accounts.pool_state.protocol_fees_a > 0 {
            // vault_a to admin_token_account_a

            let cpi_context = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault_a.to_account_info(),
                    to: ctx.accounts.admin_token_account_a.to_account_info(),
                    mint: ctx.accounts.mint_a.to_account_info(),
                    authority: ctx.accounts.pool_state.to_account_info(),
                },
                signer_seeds,
            );

            transfer_checked(
                cpi_context,
                ctx.accounts.pool_state.protocol_fees_a,
                ctx.accounts.mint_a.decimals,
            )?;
        }

        if ctx.accounts.pool_state.protocol_fees_b > 0 {
            // vault_b to admin_token_account_b

            let cpi_context = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault_b.to_account_info(),
                    to: ctx.accounts.admin_token_account_b.to_account_info(),
                    mint: ctx.accounts.mint_b.to_account_info(),
                    authority: ctx.accounts.pool_state.to_account_info(),
                },
                signer_seeds,
            );

            transfer_checked(
                cpi_context,
                ctx.accounts.pool_state.protocol_fees_b,
                ctx.accounts.mint_b.decimals,
            )?;
        }

        ctx.accounts.pool_state.protocol_fees_a = 0;
        ctx.accounts.pool_state.protocol_fees_b = 0;

        Ok(())
    }

    pub fn collect_fund_fee(ctx: Context<CollectFundFee>) -> Result<()> {
        check_context(&ctx)?;

        let mint_a_key = ctx.accounts.mint_a.key();
        let mint_b_key = ctx.accounts.mint_b.key();
        let amm_cfg_key = ctx.accounts.amm_config.key();

        let seeds = [
            b"pool_state",
            mint_a_key.as_ref(),
            mint_b_key.as_ref(),
            amm_cfg_key.as_ref(),
            &[ctx.accounts.pool_state.pool_bump],
        ];

        let signer_seeds = &[&seeds[..]];

        if ctx.accounts.pool_state.fund_fees_a > 0 {
            // vault_a to admin_token_account_a

            let cpi_context = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault_a.to_account_info(),
                    to: ctx.accounts.admin_token_account_a.to_account_info(),
                    mint: ctx.accounts.mint_a.to_account_info(),
                    authority: ctx.accounts.pool_state.to_account_info(),
                },
                signer_seeds,
            );

            transfer_checked(
                cpi_context,
                ctx.accounts.pool_state.fund_fees_a,
                ctx.accounts.mint_a.decimals,
            )?;
        }

        if ctx.accounts.pool_state.fund_fees_b > 0 {
            // vault_b to admin_token_account_b

            let cpi_context = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault_b.to_account_info(),
                    to: ctx.accounts.admin_token_account_b.to_account_info(),
                    mint: ctx.accounts.mint_b.to_account_info(),
                    authority: ctx.accounts.pool_state.to_account_info(),
                },
                signer_seeds,
            );

            transfer_checked(
                cpi_context,
                ctx.accounts.pool_state.fund_fees_b,
                ctx.accounts.mint_b.decimals,
            )?;
        }

        ctx.accounts.pool_state.fund_fees_a = 0;
        ctx.accounts.pool_state.fund_fees_b = 0;

        Ok(())
    }

    pub fn update_pool_status(ctx: Context<UpdatePoolStatus>, status: bool) -> Result<()> {
        ctx.accounts.pool_state.status = status;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct UpdatePoolStatus<'info> {
    #[account(mut, constraint = admin.key() == operation_state.admin_key @ ApexFlowError::UnauthorizedAdmin)]
    pub admin: Signer<'info>,

    #[account(seeds = [b"operation_state"], bump)]
    pub operation_state: Account<'info, OperationState>,

    pub amm_config: Account<'info, AmmConfig>,

    #[account(constraint = mint_a.key() == pool_state.mint_a @ ApexFlowError::InvalidMint)]
    pub mint_a: Account<'info, Mint>,

    #[account(constraint = mint_b.key() == pool_state.mint_b @ ApexFlowError::InvalidMint)]
    pub mint_b: Account<'info, Mint>,

    #[account(mut, seeds = [b"pool_state", mint_a.key().as_ref(), mint_b.key().as_ref(), amm_config.key().as_ref()], bump )]
    pub pool_state: Account<'info, PoolState>,
}

#[derive(Accounts)]
pub struct CollectProtocolFee<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(constraint = admin.key() == amm_config.admin_key @ ApexFlowError::UnauthorizedAdmin)]
    pub amm_config: Box<Account<'info, AmmConfig>>,

    #[account(constraint = mint_a.key() == pool_state.mint_a @ ApexFlowError::InvalidMint)]
    pub mint_a: Account<'info, Mint>,

    #[account(constraint = mint_b.key() == pool_state.mint_b @ ApexFlowError::InvalidMint)]
    pub mint_b: Account<'info, Mint>,

    #[account(mut, seeds = [b"pool_state", mint_a.key().as_ref(), mint_b.key().as_ref(), amm_config.key().as_ref()], bump )]
    pub pool_state: Box<Account<'info, PoolState>>,

    #[account(mut, associated_token::mint = mint_a, associated_token::authority = pool_state,
        constraint = vault_a.key() == pool_state.vault_a @ ApexFlowError::InvalidVault)]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(mut, associated_token::mint = mint_b, associated_token::authority = pool_state,
        constraint = vault_b.key() == pool_state.vault_b @ ApexFlowError::InvalidVault)]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(associated_token::mint = mint_a, associated_token::authority = admin)]
    pub admin_token_account_a: Account<'info, TokenAccount>,

    #[account(associated_token::mint = mint_b, associated_token::authority = admin)]
    pub admin_token_account_b: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CollectFundFee<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(constraint = admin.key() == amm_config.admin_key @ ApexFlowError::UnauthorizedAdmin)]
    pub amm_config: Box<Account<'info, AmmConfig>>,

    #[account(constraint = mint_a.key() == pool_state.mint_a @ ApexFlowError::InvalidMint)]
    pub mint_a: Account<'info, Mint>,

    #[account(constraint = mint_b.key() == pool_state.mint_b @ ApexFlowError::InvalidMint)]
    pub mint_b: Account<'info, Mint>,

    #[account(mut, seeds = [b"pool_state", mint_a.key().as_ref(), mint_b.key().as_ref(), amm_config.key().as_ref()], bump )]
    pub pool_state: Box<Account<'info, PoolState>>,

    #[account(mut, associated_token::mint = mint_a, associated_token::authority = pool_state,
        constraint = vault_a.key() == pool_state.vault_a @ ApexFlowError::InvalidVault)]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(mut, associated_token::mint = mint_b, associated_token::authority = pool_state,
        constraint = vault_b.key() == pool_state.vault_b @ ApexFlowError::InvalidVault)]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(associated_token::mint = mint_a, associated_token::authority = admin)]
    pub admin_token_account_a: Account<'info, TokenAccount>,

    #[account(associated_token::mint = mint_b, associated_token::authority = admin)]
    pub admin_token_account_b: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(constraint = mint_a.key() == pool_state.mint_a @ ApexFlowError::InvalidMint)]
    pub mint_a: Account<'info, Mint>,
    #[account(constraint = mint_b.key() == pool_state.mint_b @ ApexFlowError::InvalidMint)]
    pub mint_b: Account<'info, Mint>,

    #[account(mut, seeds = [b"pool_state", mint_a.key().as_ref(), mint_b.key().as_ref(), amm_config.key().as_ref()], bump )]
    pub pool_state: Box<Account<'info, PoolState>>,
    #[account(constraint = amm_config.key() == pool_state.amm_config @ ApexFlowError::InvalidAmmConfigStatus)]
    pub amm_config: Account<'info, AmmConfig>,

    #[account(mut, associated_token::mint = mint_a, associated_token::authority = user)]
    pub input_token_account: Account<'info, TokenAccount>,

    #[account(mut, associated_token::mint = mint_b, associated_token::authority = user)]
    pub output_token_account: Account<'info, TokenAccount>,

    #[account(mut, associated_token::mint = mint_a, associated_token::authority = pool_state,
        constraint = vault_a.key() == pool_state.vault_a @ ApexFlowError::InvalidVault)]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(mut, associated_token::mint = mint_b, associated_token::authority = pool_state,
        constraint = vault_b.key() == pool_state.vault_b @ ApexFlowError::InvalidVault)]
    pub vault_b: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(constraint = mint_a.key() == pool_state.mint_a @ ApexFlowError::InvalidMint)]
    pub mint_a: Box<Account<'info, Mint>>,
    #[account(constraint = mint_b.key() == pool_state.mint_b @ ApexFlowError::InvalidMint)]
    pub mint_b: Box<Account<'info, Mint>>,

    #[account(seeds = [b"pool_state", mint_a.key().as_ref(), mint_b.key().as_ref(), amm_config.key().as_ref()], bump )]
    pub pool_state: Box<Account<'info, PoolState>>,
    #[account(constraint = amm_config.key() == pool_state.amm_config @ ApexFlowError::InvalidAmmConfigStatus)]
    pub amm_config: Box<Account<'info, AmmConfig>>,

    #[account(mut, associated_token::mint = mint_a, associated_token::authority = user)]
    pub user_token_account_a: Box<Account<'info, TokenAccount>>,

    #[account(mut, associated_token::mint = mint_b, associated_token::authority = user)]
    pub user_token_account_b: Box<Account<'info, TokenAccount>>,

    #[account(mut, associated_token::mint = mint_a, associated_token::authority = pool_state,
        constraint = vault_a.key() == pool_state.vault_a @ ApexFlowError::InvalidVault)]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(mut, associated_token::mint = mint_b, associated_token::authority = pool_state,
        constraint = vault_b.key() == pool_state.vault_b @ ApexFlowError::InvalidVault)]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(mut, mint::authority = pool_state, seeds = [b"lp_mint", pool_state.key().as_ref()], bump )]
    pub lp_mint: Box<Account<'info, Mint>>,

    #[account(mut, associated_token::mint = lp_mint, associated_token::authority = user)]
    pub user_lp_mint_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(constraint = mint_a.key() == pool_state.mint_a @ ApexFlowError::InvalidMint)]
    pub mint_a: Box<Account<'info, Mint>>,
    #[account(constraint = mint_b.key() == pool_state.mint_b @ ApexFlowError::InvalidMint)]
    pub mint_b: Box<Account<'info, Mint>>,

    #[account(seeds = [b"pool_state", mint_a.key().as_ref(), mint_b.key().as_ref(), amm_config.key().as_ref()], bump )]
    pub pool_state: Box<Account<'info, PoolState>>,
    #[account(constraint = amm_config.key() == pool_state.amm_config @ ApexFlowError::InvalidAmmConfigStatus)]
    pub amm_config: Box<Account<'info, AmmConfig>>,

    #[account(mut, associated_token::mint = mint_a, associated_token::authority = user)]
    pub user_token_account_a: Box<Account<'info, TokenAccount>>,

    #[account(mut, associated_token::mint = mint_b, associated_token::authority = user)]
    pub user_token_account_b: Box<Account<'info, TokenAccount>>,

    #[account(mut, associated_token::mint = mint_a, associated_token::authority = pool_state,
        constraint = vault_a.key() == pool_state.vault_a @ ApexFlowError::InvalidVault)]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(mut, associated_token::mint = mint_b, associated_token::authority = pool_state,
        constraint = vault_b.key() == pool_state.vault_b @ ApexFlowError::InvalidVault)]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(mut, mint::authority = pool_state, seeds = [b"lp_mint", pool_state.key().as_ref()], bump )]
    pub lp_mint: Account<'info, Mint>,

    #[account(init_if_needed, payer = user, associated_token::mint = lp_mint, associated_token::authority = user)]
    pub user_lp_mint_account: Account<'info, TokenAccount>,

    #[account(constraint = dead_address.key() == anchor_lang::solana_program::system_program::ID)]
    /// CHECK: this is the dead address used to permanently lock minimum liquidity
    pub dead_address: UncheckedAccount<'info>,

    #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = lp_mint,
    associated_token::authority = dead_address
)]
    pub minimum_liquidity_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(constraint = amm_config.status @ ApexFlowError::InvalidAmmConfigStatus)]
    pub amm_config: Account<'info, AmmConfig>,

    pub mint_a: Account<'info, Mint>,
    pub mint_b: Account<'info, Mint>,

    #[account(init,
        payer = creator,
        space = 8 + PoolState::INIT_SPACE,
        seeds = [b"pool_state", mint_a.key().as_ref(), mint_b.key().as_ref(), amm_config.key().as_ref()], bump)]
    pub pool_state: Account<'info, PoolState>,

    #[account(init, payer = creator, associated_token::mint = mint_a, associated_token::authority = pool_state )]
    pub vault_a: Account<'info, TokenAccount>,
    #[account(init, payer = creator, associated_token::mint = mint_b, associated_token::authority = pool_state )]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(init, payer = creator, mint::decimals = 9,
        mint::authority = pool_state, seeds = [b"lp_mint", pool_state.key().as_ref()], bump )]
    pub lp_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeOperationState<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(init, payer = signer, space = 8 + OperationState::INIT_SPACE, seeds = [b"operation_state"], bump )]
    pub operation_state: Account<'info, OperationState>,

    pub apex_flow_program: Program<'info, crate::program::ApexFlow>,

    #[account(constraint = apex_flow_program_data.upgrade_authority_address == Some(signer.key()) @ ApexFlowError::UnauthorizedAdmin)]
    pub apex_flow_program_data: Account<'info, ProgramData>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(fee_index: u8)]
pub struct InitializeAmmConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(init, payer = admin, space = 8 + AmmConfig::INIT_SPACE, seeds = [b"amm_config", fee_index.to_le_bytes().as_ref()], bump )]
    pub amm_config: Account<'info, AmmConfig>,

    #[account(seeds = [b"operation_state"], bump)]
    pub operation_state: Account<'info, OperationState>,

    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct PoolState {
    pub amm_config: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub lp_mint: Pubkey,
    pub vault_a: Pubkey,
    pub vault_b: Pubkey,
    pub protocol_fees_a: u64,
    pub protocol_fees_b: u64,
    pub fund_fees_a: u64,
    pub fund_fees_b: u64,
    pub pool_bump: u8,
    pub lp_mint_bump: u8,
    pub status: bool,
}

#[account]
#[derive(InitSpace)]
pub struct OperationState {
    pub admin_key: Pubkey,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct AmmConfig {
    pub admin_key: Pubkey,
    pub protocol_fee_rate: u64,
    pub fund_fee_rate: u64,
    pub trade_fee_rate: u64,
    pub status: bool,
    pub fee_index: u8,
    pub bump: u8,
}
