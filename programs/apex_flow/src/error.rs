use anchor_lang::error_code;

#[error_code]
pub enum ApexFlowError {
    // ── AmmConfig ──────────────────────────────────────────
    #[msg("Config index exceeds maximum allowed value of 3")]
    InvalidConfigIndex,

    #[msg("Provided fee rate is not a supported tier")]
    InvalidFeeRate,

    #[msg("Caller is not authorized to perform this operation")]
    UnauthorizedAdmin,

    #[msg("Amm config is disabled")]
    InvalidAmmConfigStatus,

    #[msg(
        "Invalid program id. For using program from another account please update id in the code"
    )]
    InvalidProgramId, // 6011 0x177b

    #[msg("Unexpected account")]
    UnexpectedAccount, // 6012 0x177c

    // ── Pool ───────────────────────────────────────────────
    #[msg("Token mint addresses must be provided in ascending order")]
    InvalidMintOrder,

    #[msg("Provided mint does not match pool state")]
    InvalidMint,

    #[msg("Pool is currently disabled")]
    InvalidPoolStatus,

    #[msg("Pool is not yet open for trading")]
    PoolNotOpen,

    // ── Deposit / Withdraw ─────────────────────────────────
    #[msg("Liquidity amount must be greater than zero")]
    InvalidLiquidity,

    #[msg("Deposit exceeds maximum allowed token amount")]
    ExceededMaximumDepositAmount,

    #[msg("Withdraw falls below minimum required token amount")]
    BelowMinimumWithdrawAmount,

    #[msg("Insufficient LP token balance for withdrawal")]
    InsufficientLpBalance,

    #[msg("Initial liquidity deposit must exceed minimum threshold")]
    InsufficientInitialLiquidity,

    // ── Swap ───────────────────────────────────────────────
    #[msg("Swap input amount must be greater than zero")]
    InvalidSwapAmount,

    #[msg("Output amount falls below specified minimum threshold")]
    ExceededSlippageTolerance,

    #[msg("Input amount exceeds specified maximum threshold")]
    ExceededMaximumInputAmount,

    #[msg("Insufficient liquidity in pool to fulfill swap")]
    InsufficientPoolLiquidity,

    // ── Math ───────────────────────────────────────────────
    #[msg("Arithmetic overflow during calculation")]
    MathOverflow,

    #[msg("Division by zero encountered")]
    DivisionByZero,

    #[msg("Calculation resulted in zero output")]
    ZeroOutput,

    // ── Vault / Token ──────────────────────────────────────
    #[msg("Vault authority does not match pool state")]
    InvalidVaultAuthority,

    #[msg("Provided vault does not match pool state")]
    InvalidVault,

    #[msg("Token program does not match expected program")]
    InvalidTokenProgram,
}
