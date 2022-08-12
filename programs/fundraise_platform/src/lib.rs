use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_lang::solana_program::system_instruction::transfer;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

declare_id!("7gNSLTU9NJzEhZHJUSH3ArJ9Z2gLQfxKJJZZFs32LvrA");

const FUNDRAISE_CAMPAIGN_CAP: usize = 10;

#[program]
pub mod fundraise_platform {

    use super::*;
    use std::u64;

    pub fn initialize(
        ctx: Context<Initialize>,
        goal: u64,
        commission: u64,
        chrt_reward: u64,
        n_period: i64,
        chrt_comsn_exempt_threshold: u64,
        chrt_camp_close_threshold: u64,
    ) -> Result<()> {
        require!(goal > 0, FundraiseErrors::ZeroLamports);
        let fundraise_platform = &mut ctx.accounts.fundraise_platform;
        fundraise_platform.authority = ctx.accounts.authority.key();
        fundraise_platform.goal = goal;
        fundraise_platform.raised = 0;
        fundraise_platform.id_counter = 0;

    
        // TODO: reward top10
        // TODO: reward refferral
        fundraise_platform.chrt_reward = chrt_reward;
        fundraise_platform.n_period = n_period;
        fundraise_platform.commission = commission;
        fundraise_platform.chrt_comsn_exempt_threshold = chrt_comsn_exempt_threshold;
        fundraise_platform.chrt_camp_close_threshold = chrt_camp_close_threshold;

        let top_ten_contributors = &mut ctx.accounts.top_ten_contributors;
        top_ten_contributors.contributors = vec![];

        let current_time = Clock::get()?.unix_timestamp;
        fundraise_platform.n_period_cooldown = current_time + n_period;

        Ok(())
    }

    pub fn create_new_campaign(ctx: Context<NewCampaign>) -> Result<()> {
        let campaign_acc = &mut ctx.accounts.campaign_acc;
        campaign_acc.campaign_authority = ctx.accounts.campaign_authority.key();
        // TODO: cleanup unused args
        campaign_acc.is_commission_free = false;
        // campaign_acc.is_active = false;
        campaign_acc.chrt_recieved = 0;

        // TODO: add max active campaigns

        Ok(())
    }

    pub fn contribute(
        ctx: Context<Contribute>,
        id: u64,
        amount: u64,
        referrer: Pubkey,
        bump: u8,
    ) -> Result<()> {
        require!(amount > 0, FundraiseErrors::ZeroLamports);
        let fundraise_platform = &ctx.accounts.fundraise_platform;
        require!(
            id <= fundraise_platform.id_counter,
            FundraiseErrors::IDGreaterThanCounter
        );

        let campaign_acc = &mut ctx.accounts.campaign_acc;
        // require!(campaign_acc.is_active, FundraiseErrors::CampaignNotActive);

        let contributor = &ctx.accounts.contributor;

        let chrt_to_receive = &mut ctx.accounts.referrer_acc;

        let raised = fundraise_platform.raised;
        let goal = fundraise_platform.goal;
        require!(goal > raised, FundraiseErrors::GoalAchieved);

        let (from, from_info) = (&contributor.key(), contributor.to_account_info());
        let (to, to_info) = (
            &fundraise_platform.key(),
            fundraise_platform.to_account_info(),
        );
        let is_commission_free =
            campaign_acc.chrt_recieved < fundraise_platform.chrt_comsn_exempt_threshold;
        // commission is in percent, so we need to divide amount by 100.
        let commission_percentage = amount / 100 * fundraise_platform.commission;
        let fee: u64 = if is_commission_free {
            commission_percentage
        } else {
            0
        };
        let rough_amount = amount - commission_percentage;

        invoke(&transfer(from, to, rough_amount), &[from_info, to_info])?;

        if fee > 0 {
            let commission_ix = transfer(from, to, commission_percentage);

            invoke(
                &commission_ix,
                &[
                    contributor.to_account_info(),
                    fundraise_platform.to_account_info(),
                ],
            )?;
        }

        let fundraise_platform = &mut ctx.accounts.fundraise_platform;
        let contributor_acc = &mut ctx.accounts.contributor_acc;

        let mut _id = id;
        if _id == 0 {
            _id = fundraise_platform.id_counter;
        }

        if _id == fundraise_platform.id_counter {
            contributor_acc.address = ctx.accounts.contributor.key();
            contributor_acc.amount = 0;

            fundraise_platform.id_counter += 1;
        }
        fundraise_platform
            .raised_campaign_amounts
            .push(RaisedCampaignAmount {
                amount,
                campaign_id: _id,
            });

        contributor_acc.amount += amount;
        contributor_acc.referrer.push(referrer);
        fundraise_platform.commission += fee;
        fundraise_platform.raised += rough_amount;

        let top_ten_contributors = &mut ctx.accounts.top_ten_contributors;
        let (current_balance, mut current_i) = (contributor_acc.amount, 0);
        let (mut min, mut min_i) = (u64::MAX, TopTenContributors::MAX_CONTRIBUTORS);
        let mut found = false;
        for (i, cont) in top_ten_contributors.contributors.iter().enumerate() {
            if cont.amount < min {
                min = cont.amount;
                min_i = i;
            }
            if cont.address == contributor_acc.address {
                current_i = i;
                found = true;
                break;
            }
        }

        if !found {
            let contributor_instance = ContributorStruct {
                amount: contributor_acc.amount,
                address: contributor_acc.address,
            };

            if top_ten_contributors.contributors.len() < TopTenContributors::MAX_CONTRIBUTORS {
                top_ten_contributors.contributors.push(contributor_instance);
            } else if min < current_balance {
                top_ten_contributors.contributors[min_i] = contributor_instance;
            }
        } else {
            top_ten_contributors.contributors[current_i].amount = current_balance;
        }
        /* ---------- referral reward logic----------- */
        // SOL to CHRT ratio
        let lamps_2_chrt = amount * 101;

        let referrer_acc_data = chrt_to_receive.to_account_info();
        anchor_spl::token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::MintTo {
                    mint: ctx.accounts.mint_acc.to_account_info(),
                    to: referrer_acc_data,
                    authority: ctx.accounts.mint_acc.to_account_info(),
                },
                &[&["chrt_mint".as_bytes(), &[bump]]],
            ),
            lamps_2_chrt,
        )?;
        /* ----------end of referral reward logic----------- */

        /* -----------reward top 10 donators-------------*/
        // TODO: reward top 10 donators
        let current_time = Clock::get()?.unix_timestamp;
        if current_time >= fundraise_platform.n_period_cooldown {
            let mut destination = **ctx.accounts.destination;

            while !top_ten_contributors.contributors.is_empty() {
                destination.owner = top_ten_contributors.contributors[0].address;

                anchor_spl::token::mint_to(
                    CpiContext::new_with_signer(
                        ctx.accounts.token_program.to_account_info(), //##11
                        anchor_spl::token::MintTo {
                            mint: ctx.accounts.mint_acc.to_account_info(),
                            to: ctx.accounts.destination.to_account_info(),
                            authority: ctx.accounts.mint_acc.to_account_info(),
                        },
                        &[&["faucet-mint".as_bytes(), &[bump]]],
                    ),
                    fundraise_platform.chrt_reward,
                )?;

                top_ten_contributors.contributors.remove(0);
            }
            fundraise_platform.n_period_cooldown = current_time + fundraise_platform.n_period;
        }
        /* -----------end ofreward top 10 donators-------------*/

        emit!(ContributionEvent {
            at: Clock::get()?.unix_timestamp,
            amount,
            platform_after: fundraise_platform.raised,
            from: contributor_acc.address,
            referrer,
        });
        Ok(())
    }

    pub fn contribute_chrt(
        ctx: Context<ContributeCHRT>,
        amount: u64,
        _id: u64,
        is_commission_free: bool,
    ) -> Result<()> {
        let campaign_acc = &mut ctx.accounts.campaign_acc;
        let campaign_token_acc = &ctx.accounts.campaign_chrt_acc;

        let contributor = &ctx.accounts.contributor;
        let contributor_chrt = &ctx.accounts.contributor_chrt_acc;

        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::Transfer {
                    from: contributor_chrt.to_account_info(),
                    to: campaign_token_acc.to_account_info(),
                    authority: contributor.to_account_info(),
                },
                &[&["faucet-mint".as_bytes()]],
            ),
            amount,
        )?;

        if is_commission_free {
            campaign_acc.chrt_recieved += amount;
        } else {
            campaign_acc.chrt_cover_goal += amount;
        }

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        let campaign_acc_owner = &mut ctx.accounts.authority;
        let campaign_acc = &mut ctx.accounts.campaign_acc;

        let raised = ctx.accounts.fundraise_platform.raised;
        require!(raised > 0, FundraiseErrors::ZeroLamportsRaised);
        require!(
            campaign_acc.campaign_authority == campaign_acc_owner.key(),
            FundraiseErrors::NotTheOwner
        );

        let from = campaign_acc.to_account_info();
        let to = campaign_acc_owner.to_account_info();

        let rent_exemption = Rent::get()?.minimum_balance(Funds::SIZE);
        let withdraw_amount = **from.lamports.borrow() - rent_exemption;
        // require!(withdraw_amount < raised, FundraiseErrors::InsufficientBalance);

        **from.try_borrow_mut_lamports()? -= withdraw_amount;
        **to.try_borrow_mut_lamports()? += withdraw_amount;
        ctx.accounts.fundraise_platform.raised = 0;

        emit!(WithdrawEvent {
            at: Clock::get()?.unix_timestamp,
            amount: withdraw_amount
        });

        Ok(())
    }

    pub fn end_fundraise(ctx: Context<EndFundraise>, id: u64) -> Result<()> {
        let fundraise_platform = &mut ctx.accounts.fundraise_platform;
        let campaign_acc = &mut ctx.accounts.campaign_acc;
        
        require!(
            !campaign_acc.is_active,
            FundraiseErrors::CampaignNotActive
        );
        require!(
            fundraise_platform.chrt_camp_close_threshold > campaign_acc.chrt_cover_goal,
            FundraiseErrors::InsufficientTokens
        );
        campaign_acc.is_active = true;
        let curr_raised_amount = fundraise_platform
            .raised_campaign_amounts
            .binary_search_by(|x| x.campaign_id.cmp(&id))
            .unwrap();

        let redistribute_amount =
            fundraise_platform.raised_campaign_amounts[curr_raised_amount].amount;
        fundraise_platform
            .raised_campaign_amounts
            .remove(curr_raised_amount);

        let sum: u128 = fundraise_platform
            .raised_campaign_amounts
            .iter()
            .map(|x| x.amount as u128)
            .sum();

        for current_amount in &mut fundraise_platform.raised_campaign_amounts {
            current_amount.amount +=
                (redistribute_amount as u128 * current_amount.amount as u128 / sum) as u64;
        }

        Ok(())
    }

    pub fn withdraw_commission(ctx: Context<WithdrawCommission>) -> Result<()> {
        let fundraise_platform_account = &mut ctx.accounts.fundraise_platform;
        let fundraise_platform_owner = &mut ctx.accounts.authority;

        require!(
            fundraise_platform_owner.key() == fundraise_platform_account.authority,
            FundraiseErrors::NotTheOwner
        );
        
        let from = fundraise_platform_account.to_account_info();
        let to = fundraise_platform_owner.to_account_info();

        **from
            .try_borrow_mut_lamports()? -= fundraise_platform_account.commission;
        **to.try_borrow_mut_lamports()? +=
            fundraise_platform_account.commission;

        fundraise_platform_account.commission = 0;

        Ok(())
    }

    // pub fn reward_top_donators (ctx: Context<Contribute>)
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
    pub fundraise_platform: Box<Account<'info, Funds>>,

    #[account(
        init,
        payer = authority,
        space = TopTenContributors::SIZE,
        seeds = [b"top_ten_contributors", authority.key().as_ref()],
        bump
    )]
    pub top_ten_contributors: Account<'info, TopTenContributors>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct NewCampaign<'info> {
    #[account(
        init,
        payer = campaign_authority,
        space = Funds::SIZE,
        seeds = [b"campaign",campaign_authority.key().as_ref()],
        bump)]
    pub campaign_acc: Account<'info, FundraiseAccount>,
    #[account(mut)]
    pub campaign_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority,
        seeds = [b"fundraise_platform", fundraise_platform.authority.key().as_ref()],
        bump
    )]
    pub fundraise_platform: Account<'info, Funds>,
    #[account(mut, seeds=[b"campaign", id.to_string().as_bytes(), campaign_acc.campaign_authority.key().as_ref()], bump)]
    pub campaign_acc: Box<Account<'info, FundraiseAccount>>,
    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
#[instruction(id: u64, amount: u64)]
pub struct Contribute<'info> {
    #[account(mut)]
    pub contributor: Signer<'info>,
    pub system_program: Program<'info, System>,

    #[account(
        init_if_needed,
        payer = contributor,
        space = Contributor::SIZE,
        seeds = [
            b"fundraise_platform_contributor",
            fundraise_platform.key().as_ref(),
            id.to_string().as_bytes()
        ],
        bump
    )]
    pub contributor_acc: Account<'info, Contributor>,

    #[account(
        mut,
        seeds = [b"fundraise_platform", fundraise_platform.authority.key().as_ref()],
        bump
    )]
    pub fundraise_platform: Box<Account<'info, Funds>>,

    #[account(
        mut,
        seeds = [b"top_ten_contributors", fundraise_platform.authority.key().as_ref()],
        bump
    )]
    pub top_ten_contributors: Account<'info, TopTenContributors>,
    // mint
    #[account(mut, seeds=[b"campaign", id.to_string().as_bytes(), campaign_acc.campaign_authority.key().as_ref()], bump)]
    pub campaign_acc: Box<Account<'info, FundraiseAccount>>,
    #[account(
        init_if_needed,
        payer = minter,
        seeds = [b"chrt_mint".as_ref()],
        bump,
        mint::decimals = 3,
        mint::authority = mint_acc
    )]
    pub mint_acc: Account<'info, Mint>, //##8

    #[account(
        init_if_needed,
        payer = minter,
        associated_token::mint = mint_acc,
        associated_token::authority = adestination
    )]
    pub destination: Account<'info, TokenAccount>,
    #[account(mut)]
    pub adestination: AccountInfo<'info>,
    #[account(mut)]
    pub minter: Signer<'info>,
    #[account(
        init_if_needed,
        payer = minter,
        associated_token::mint = mint_acc,
        associated_token::authority = areceiver
    )]
    pub referrer_acc: Account<'info, TokenAccount>,
    #[account(mut)]
    pub areceiver: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
#[instruction(amount: u64, id: u64,)]
pub struct ContributeCHRT<'info> {
    #[account(mut)]
    pub contributor: Signer<'info>,
    #[account(mut)]
    pub contributor_chrt_acc: Account<'info, TokenAccount>,
    #[account(mut)]
    pub campaign_chrt_acc: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"fundraise_platform", fundraise_platform.authority.key().as_ref()],
        bump
    )]
    pub fundraise_platform: Box<Account<'info, Funds>>,
    #[account(mut, seeds=[b"campaign", id.to_string().as_bytes(), campaign_acc.campaign_authority.key().as_ref()], bump)]
    pub campaign_acc: Box<Account<'info, FundraiseAccount>>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct EndFundraise<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority,
        seeds = [b"fundraise_platform", fundraise_platform.authority.key().as_ref()],
        bump
    )]
    pub fundraise_platform: Account<'info, Funds>,
    #[account(mut, seeds=[b"campaign", id.to_string().as_bytes()], bump)]
    pub campaign_acc: Box<Account<'info, FundraiseAccount>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawCommission<'info> {
    #[account(mut, seeds = [b"fundraise_platform", fundraise_platform.authority.key().as_ref()], bump)]
    pub fundraise_platform: Account<'info, Funds>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct FundraiseAccount {
    pub campaign_authority: Pubkey,
    pub is_commission_free: bool, //commission_exempt
    pub is_active: bool,
    pub chrt_recieved: u64,
    pub chrt_cover_goal: u64,
    pub raised_after_commission: u64, // pub camp_portion: u64, //raised
}

impl FundraiseAccount {
    pub const SIZE: usize = 8 + 32 + 2 + 8 + 8;
}
#[account]
pub struct Funds {
    pub authority: Pubkey,
    pub goal: u64,
    pub raised: u64,
    pub id_counter: u64,
    // commission % and refferral
    pub n_period: i64,
    pub n_period_cooldown: i64,
    pub commission: u64,
    pub chrt_reward: u64,                 //encrg_chrt
    pub chrt_comsn_exempt_threshold: u64, //lim_chrt_comm_exempt
    pub chrt_camp_close_threshold: u64,   //lim_chrt_camp_close
    pub raised_campaign_amounts: Vec<RaisedCampaignAmount>,
    pub compleated_campaigns: u32, //finished_camp_numbers
}
impl Funds {
    pub const SIZE: usize = 8 + 32 + 8 * 9 + 4 + (4 + 16 * FUNDRAISE_CAMPAIGN_CAP);
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct RaisedCampaignAmount {
    pub amount: u64,
    pub campaign_id: u64,
}
#[account]
pub struct Contributor {
    pub address: Pubkey,
    pub amount: u64,
    pub referrer: Vec<Pubkey>,
}

impl Contributor {
    pub const SIZE: usize = 8 + (32 * 2) + 4 + 8;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct ContributorStruct {
    pub address: Pubkey,
    pub amount: u64,
}

#[account]
pub struct TopTenContributors {
    pub contributors: Vec<ContributorStruct>,
}

impl TopTenContributors {
    pub const MAX_CONTRIBUTORS: usize = 10;
    pub const SIZE: usize = 8 + 4 + (32 + 8) * TopTenContributors::MAX_CONTRIBUTORS;
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
    IDGreaterThanCounter,
    #[msg("You are not the owner of this account!")]
    NotTheOwner,
    #[msg("The campaign is no longer active!")]
    CampaignNotActive,
    #[msg("Not enough CHRT!")]
    InsufficientTokens,
}

#[event]
pub struct ContributionEvent {
    at: i64,
    amount: u64,
    platform_after: u64,
    from: Pubkey,
    referrer: Pubkey,
}

#[event]
pub struct WithdrawEvent {
    at: i64,
    amount: u64,
}
