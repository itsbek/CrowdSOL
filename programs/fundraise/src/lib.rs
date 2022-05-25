use anchor_lang::prelude::*;
use anchor_lang::solana_program::pubkey;
use anchor_lang::solana_program::system_instruction::transfer;
use anchor_lang::solana_program::program::invoke;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod fundraise {
    use:: std::u64;
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, goal: u64) -> Result<()> {
        require!(goal > 0, FundraiseErrors::ZeroLamports);
        let fundraise_platform = &mut ctx.accounts.fundraise_platform;
        fundraise_platform.authority = ctx.accounts.authority.key();
        fundraise_platform.goal = goal;
        fundraise_platform.collected = 0;
        fundraise_platform.id_counter = 0;

        let top_donators = &mut ctx.accounts.top_donators;
        top_donators.funders = vec![];

        Ok(())
    
    }
    pub fn fund(ctx: Context<Fund>, id: u64, amount: u64) -> Result<()>{

    }
}

#[derive(Accounts)]
#[instruction(goal: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = Funds::SIZE,
        seeds = [b"fundraise_platform", authority.key().as_ref()],
        bump
    )]
    pub fundraise_platform:Account<'info, Funds>,

    #[account(
        init,
        payer = authority,
        space = TopDonators::SIZE,
        seeds = [b"top_donators", authority.key().as_ref()],
        bump
    )]
    pub top_donators: Account<'info, TopDonators>,
    pub system_program: Program<'info, System>
}

#[derive(Accounts)]
#[instruction(id: u64, amount: u64)]
pub struct Witdraw<'info>{
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut,
        has_one = authority,
        seeds = [b"fundraise_platform", fundraise_platform.authority.key().as_ref()],
        bump
    )]
    pub fundraise_platform: Account<'info, Funds>,
    pub system_program: Program<'info, System>
}

#[derive(Accounts)]
#[instruction(id: u64, amount: u64)]
pub struct Fund<'info>{
    #[account(mut)]
    pub donator: Signer<'info>,
    pub system_program: Program<'info, System>,
    
    #[account(
        init_if_needed,
        payer = funder,
        space = Funder:SIZE,
        seeds = [b"fundraise_platform", fundraise_platform.authority.key().as_ref(), id.to_string().as_bytes()],
        bump
    )]
    pub funder_account: Account<'info, Funder>,

    #[account(
        mut,
        seeds = [b"fundraise_platform", fundraise_platform.authority.as_key().as_key()],
        bump
    )]
    pub fundraise_platform: Account<'info, Funds>,

    #[account(
        mut,
        seeds = [b"top_donators", fundraise_platform.authority.key().as_ref()],
        bump
    )]
    pub top_donators: Account<'info, TopDonators>
}

#[account]
pub struct Funds{
    pub authority: Pubkey,
    pub goal: u64,
    pub collected: u64,
    pub id_counter: u64
}

impl Funds {
    pub const SIZE: usize = 8+32+8*3;
}

#[account]
pub struct Funder {
    pub address: Pubkey,
    pub amount: u64,
}

impl Funder {
    pub const SIZE: usize = 8+32+8;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, )]
pub struct FunderStruct{
    pub address: Pubkey,
    pub amount: u64
}

#[account]
pub struct TopDonators {
    pub funders: Vec<FunderStruct>
}

impl TopDonators {
    pub const MAX_FUNDERS: usize = 10;
    pub const SIZE: usize = 8+4+(32+8)*TopDonators::MAX_FUNDERS;
}


// possible error scenarios to log from anchor prelude
#[error_code]
pub enum FundraiseErrors {
    #[msg("Lamports amount must be greater than zero!")]
    ZeroLamports,
    #[msg("0 amount of lamports were raised!")]
    ZeroLamportsRaised,
    #[msg("The goal was achieved!")]
    GoalAchieved,
    #[msg("Insufficient balance to pay the rent!")]
    InsufficientBalance,
    #[msg("Entered ID is greter than current ID counter")]
    IDGreaterThanCounter
}

#[event]
pub struct FundraiseEvent {
    at: i64,
    amount: u64,
    platform_after: u64,
    from: Pubkey,
}

#[event]
pub struct WithdrawEvent {
    at: i64,
    amount: u64,
}