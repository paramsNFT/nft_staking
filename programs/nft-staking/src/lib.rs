pub mod utils;

use std::cell::Ref;
use std::cmp;
use crate::constants::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{TokenAccount};
use anchor_lang::solana_program::{clock, log::sol_log};
use std::convert::Into;
use std::convert::TryInto;
use arrayref::array_ref;

const PREFIX: &str = "nft_staking";
const PREFIX_USER: &str = "nft_staking_user";
const PREFIX_MINT: &str = "nft_staking_mint";
const PRECISION: u128 = u64::MAX as u128;

declare_id!("paramKFFuRPLVXZWjDRbnk5xKemduYZUW2BqUp7xZD3");

pub mod constants {
    pub const MIN_DURATION: u64 = 1;

    pub const MAX_MINT_LIMIT: usize = 300000;

    pub const PUBKEY_SIZE: usize = 32;
}

pub fn get_config_count(data: &Ref<&mut [u8]>) -> core::result::Result<usize, ProgramError> {
    return Ok(u32::from_le_bytes(*array_ref![data, CONFIG_SIZE_START, 4]) as usize);
}

// fn is_sub<T: PartialEq>(mut haystack: &[T], needle: &[T]) -> bool {
//     if needle.len() == 0 { return true; }
//     while !haystack.is_empty() {
//         if haystack.starts_with(needle) { return true; }
//         haystack = &haystack[1..];
//     }
//     false
// }

pub fn check_mint_address(data: &Ref<&mut [u8]>, mint_address: &[u8; 32]) -> core::result::Result<bool, ProgramError> {
    let mut position = CONFIG_SIZE_START + 4;
    let mint_address_vec = mint_address.try_to_vec()?;

    loop {
        let current_mint_address = &data[position..position + PUBKEY_SIZE];
        let as_vec = current_mint_address.try_to_vec()?;
        if as_vec.starts_with(&mint_address_vec) {
            return Ok(true);
        }
        // if is_sub(&as_vec, &mint_address_vec) {
        //     return Ok(true);
        // }
        position = position + PUBKEY_SIZE;
        if position >= data.len() {
            break;
        }
    }
    return Ok(false);
}

#[program]
pub mod nft_staking {
    use spl_token::instruction::AuthorityType::AccountOwner;
    use utils::*;
    use super::*;

    // initialize staking pool
    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        _pool_bump: u8, uuid: String, num_mint: u32, _reward_bump: u8, reward_duration: u64,
    ) -> ProgramResult {
        if num_mint <= 0 {
            return Err(ErrorCode::InsufficientTokenStake.into());
        }
        if reward_duration < MIN_DURATION {
            return Err(ErrorCode::DurationTooShort.into());
        }
        msg!("initializing");

        let pool_account = &mut ctx.accounts.pool_account;

        if pool_account.is_initialized {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        pool_account.is_initialized = true;
        pool_account.authority = *ctx.accounts.authority.key;
        pool_account.paused = true; // initial status is paused
        pool_account.config = ctx.accounts.config.key();
        pool_account.reward_mint = *ctx.accounts.reward_mint.to_account_info().key;
        pool_account.reward_vault = ctx.accounts.reward_vault.key();
        pool_account.last_update_time = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
        pool_account.reward_rate_per_token = 1;
        pool_account.reward_duration = reward_duration;
        pool_account.reward_duration_end = 0;
        pool_account.token_stake_count = 0;
        pool_account.user_count = 0;

        let config = &mut ctx.accounts.config;
        config.authority = *ctx.accounts.authority.key;
        config.uuid = uuid;
        config.num_mint = num_mint;

        Ok(())
    }

    // add nft addresses into Config Account
    pub fn add_mint_addresses(
        ctx: Context<AddMintAddresses>,
        mint_addresses: Vec<Pubkey>,
        index: u32,
    ) -> ProgramResult {
        let config = &mut ctx.accounts.config;
        let account = config.to_account_info();
        let current_count = get_config_count(&account.data.borrow())?;
        let mut data = account.data.borrow_mut();

        let mut fixed_config_lines: Vec<Pubkey> = vec![];

        msg!("current count {}", current_count);

        if index > config.num_mint - 1 {
            return Err(ErrorCode::IndexGreaterThanLength.into());
        }

        for line in &mint_addresses {
            let address = line.clone();
            fixed_config_lines.push(address)
        }

        let as_vec = fixed_config_lines.try_to_vec()?;

        // remove unneeded u32 because we're just gonna edit the u32 at the front
        let serialized: &[u8] = &as_vec.as_slice()[4..];

        let position = CONFIG_SIZE_START + 4 + (index as usize) * PUBKEY_SIZE;

        msg!("position {}", position);

        let array_slice: &mut [u8] =
            &mut data[position..position + fixed_config_lines.len() * PUBKEY_SIZE];
        array_slice.copy_from_slice(serialized);

        // plug in new count.
        let new_count = (index as usize) + fixed_config_lines.len();
        data[CONFIG_SIZE_START..CONFIG_SIZE_START + 4]
            .copy_from_slice(&(new_count as u32).to_le_bytes());

        Ok(())
    }

    pub fn pause(ctx: Context<Pause>) -> ProgramResult {
        let pool_account = &mut ctx.accounts.pool_account;
        pool_account.paused = true;
        Ok(())
    }

    pub fn resume(ctx: Context<Pause>) -> ProgramResult {
        let pool_account = &mut ctx.accounts.pool_account;
        pool_account.paused = false;
        Ok(())
    }

    // add funder
    pub fn authorize_funder(ctx: Context<FunderChange>, funder_to_add: Pubkey) -> ProgramResult {
        // owner cannot be added into funders
        if funder_to_add == ctx.accounts.pool_account.authority {
            return Err(ErrorCode::FunderAlreadyAuthorized.into());
        }
        let funders = &mut ctx.accounts.pool_account.funders;
        if funders.iter().any(|x| *x == funder_to_add) {
            return Err(ErrorCode::FunderAlreadyAuthorized.into());
        }
        let default_pubkey = Pubkey::default();
        if let Some(idx) = funders.iter().position(|x| *x == default_pubkey) {
            funders[idx] = funder_to_add;
        } else {
            return Err(ErrorCode::MaxFunders.into());
        }
        Ok(())
    }

    // remove funder
    pub fn deauthorize_funder(ctx: Context<FunderChange>, funder_to_remove: Pubkey) -> ProgramResult {
        if funder_to_remove == ctx.accounts.pool_account.authority {
            return Err(ErrorCode::CannotDeauthorizePoolAuthority.into());
        }
        let funders = &mut ctx.accounts.pool_account.funders;
        if let Some(idx) = funders.iter().position(|x| *x == funder_to_remove) {
            funders[idx] = Pubkey::default();
        } else {
            return Err(ErrorCode::CannotDeauthorizeMissingAuthority.into());
        }
        Ok(())
    }

    pub fn fund(ctx: Context<Fund>, amount: u64) -> ProgramResult {
        let pool_account = &mut ctx.accounts.pool_account;
        let nft_quantity = ctx.accounts.config.num_mint;

        let now = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();

        /*
        (New funded amount + remaing amount in the pool) / Total NFT quantity / duration (seconds)
        */
        if now >= pool_account.reward_duration_end {
            msg!("amount {}", amount as u128);
            msg!("pool_account.reward_duration {}", pool_account.reward_duration as u128);
            msg!("nft_quantity {}", nft_quantity as u128);

            pool_account.reward_rate_per_token = (amount as u128)
                .checked_mul(PRECISION)
                .unwrap()
                .checked_div(pool_account.reward_duration as u128)
                .unwrap()
                .checked_div(nft_quantity as u128)
                .unwrap()
                .try_into()
                .unwrap();
            msg!("New reward rate per token {} ", pool_account.reward_rate_per_token);
        } else {
            let remaining = pool_account.reward_duration_end.checked_sub(now).unwrap();
            // remaining reward in the pool = reward rate per token * remaining time * number of token
            let leftover = (pool_account.reward_rate_per_token as u128)
                .checked_mul(remaining as u128)
                .unwrap()
                .checked_mul(nft_quantity as u128)
                .unwrap().try_into()
                .unwrap();

            msg!("Leftover {} rewards amount in the pool", leftover);

            pool_account.reward_rate_per_token = (amount as u128)
                .checked_mul(PRECISION)
                .unwrap()
                .checked_add(leftover)
                .unwrap()
                .checked_div(pool_account.reward_duration as u128)
                .unwrap()
                .checked_div(nft_quantity as u128)
                .unwrap()
                .try_into()
                .unwrap();

            msg!("New reward rate per token {} ", pool_account.reward_rate_per_token);
        }

        // Transfer reward tokens into the vault.
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.funder_vault.to_account_info(),
                to: ctx.accounts.reward_vault.to_account_info(),
                authority: ctx.accounts.funder.to_account_info(),
            },
        );

        anchor_spl::token::transfer(cpi_ctx, amount)?;

        pool_account.last_update_time = now; // update last update time as current time
        pool_account.reward_duration_end = now.checked_add(pool_account.reward_duration).unwrap(); // refresh the reward end period time

        Ok(())
    }

    // create user
    pub fn create_user(ctx: Context<CreateUser>, _user_bump: u8, _mint_staked_bump: u8, uuid: String) -> ProgramResult {
        let user_account = &mut ctx.accounts.user_account;
        user_account.pool = *ctx.accounts.pool_account.to_account_info().key;
        user_account.user = *ctx.accounts.user.key;
        user_account.reward_earned_claimed = 0;
        user_account.reward_earned_pending = 0;
        user_account.mint_staked_count = 0;
        user_account.uuid = uuid;
        user_account.mint_staked = *ctx.accounts.mint_staked.to_account_info().key;

        let mint_staked = &mut ctx.accounts.mint_staked;
        mint_staked.pool = *ctx.accounts.pool_account.to_account_info().key;
        mint_staked.user_account = *user_account.to_account_info().key;

        let pool_account = &mut ctx.accounts.pool_account;
        let now = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
        pool_account.last_update_time = now;
        pool_account.user_count = pool_account.user_count.checked_add(1).unwrap();

        Ok(())
    }

    // staking
    pub fn stake(ctx: Context<Stake>, _mint_staked_bump: u8, uuid: String) -> ProgramResult {
        let pool_account = &mut ctx.accounts.pool_account;
        if pool_account.paused || !pool_account.is_initialized {
            return Err(ErrorCode::PoolPaused.into());
        }

        let config = &mut ctx.accounts.config;
        let account = config.to_account_info();

        // check constraint = config.mint_addresses.iter().any(| x | * x == stake_from_account.mint)
        let stake_from_account = &mut ctx.accounts.stake_from_account;
        if check_mint_address(&account.data.borrow(), &stake_from_account.mint.to_bytes())? == false {
            msg!("Mint address is not stakable!");
            return Err(ErrorCode::InvalidMint.into());
        }

        let now = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
        pool_account.token_stake_count = pool_account.token_stake_count.checked_add(1).unwrap();
        pool_account.last_update_time = now;

        let user_account = &mut ctx.accounts.user_account;
        let user_opt = Some(user_account);

        update_rewards(
            pool_account,
            user_opt,
        ).unwrap();

        // update user account
        ctx.accounts.user_account.mint_staked = *ctx.accounts.mint_staked.to_account_info().key;
        ctx.accounts.user_account.mint_staked_count = ctx.accounts.user_account.mint_staked_count.checked_add(1).unwrap();
        ctx.accounts.user_account.uuid = uuid;

        // update mint staked
        if ctx.accounts.user_account.mint_staked_count == 0 {
            // no need to transfer any data from the old account
            let mint_staked = &mut ctx.accounts.mint_staked;
            mint_staked.pool = *ctx.accounts.pool_account.to_account_info().key;
            mint_staked.user_account = *ctx.accounts.user_account.to_account_info().key;
            mint_staked.mint_accounts.push(ctx.accounts.stake_from_account.key());
        } else {
            // has previous data
            let mint_staked = &mut ctx.accounts.mint_staked;
            mint_staked.pool = *ctx.accounts.pool_account.to_account_info().key;
            mint_staked.user_account = *ctx.accounts.user_account.to_account_info().key;

            let current_mint_staked = &mut ctx.accounts.current_mint_staked;
            for mint_address in &current_mint_staked.mint_accounts {
                mint_staked.mint_accounts.push(*mint_address);
            }
            mint_staked.mint_accounts.push(ctx.accounts.stake_from_account.key());
        }

        // Transfer token authority
        {
            let (pool_pda, pool_bump) = Pubkey::find_program_address(&[PREFIX.as_bytes(),
                ctx.accounts.pool_account.authority.as_ref(),
                ctx.accounts.pool_account.config.as_ref(),
            ], ctx.program_id);
            let _seeds = &[PREFIX.as_bytes(),
                ctx.accounts.pool_account.authority.as_ref(),
                ctx.accounts.pool_account.config.as_ref(),
                &[pool_bump]]; // need this to sign the pda, match the authority

            let cpi_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info().clone(),
                anchor_spl::token::SetAuthority {
                    current_authority: ctx.accounts.staker.to_account_info().clone(),
                    account_or_mint: ctx.accounts.stake_from_account.to_account_info().clone(),
                },
            );
            msg!("Calling the token program to transfer authority from staker to pool");
            anchor_spl::token::set_authority(cpi_ctx, AccountOwner, Some(pool_pda))?;
        }

        Ok(())
    }

    // unstake
    pub fn unstake(ctx: Context<Unstake>, _mint_staked_bump: u8, uuid: String) -> ProgramResult {
        let pool_account = &mut ctx.accounts.pool_account;
        if pool_account.paused || !pool_account.is_initialized {
            return Err(ErrorCode::PoolPaused.into());
        }

        let now = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
        pool_account.last_update_time = now;
        pool_account.token_stake_count = pool_account.token_stake_count.checked_sub(1).unwrap();

        let user_account = &mut ctx.accounts.user_account;
        let user_opt = Some(user_account);
        update_rewards(
            pool_account,
            user_opt,
        ).unwrap();

        ctx.accounts.user_account.mint_staked = *ctx.accounts.mint_staked.to_account_info().key;
        ctx.accounts.user_account.mint_staked_count = ctx.accounts.user_account.mint_staked_count.checked_sub(1).unwrap();
        ctx.accounts.user_account.uuid = uuid;

        // count of user_account.mint_staked must be >= 1
        let mint_staked = &mut ctx.accounts.mint_staked;
        mint_staked.pool = *ctx.accounts.pool_account.to_account_info().key;
        mint_staked.user_account = *ctx.accounts.user_account.to_account_info().key;

        let current_mint_staked = &mut ctx.accounts.current_mint_staked;
        for mint_address in &current_mint_staked.mint_accounts {
            if mint_address != ctx.accounts.unstake_from_account.to_account_info().key {
                mint_staked.mint_accounts.push(*mint_address);
            }
        }

        // Transfer token authority
        {
            let (_pool_pda, pool_bump) = Pubkey::find_program_address(&[PREFIX.as_bytes(),
                ctx.accounts.pool_account.authority.as_ref(),
                ctx.accounts.pool_account.config.as_ref(),
            ], ctx.program_id);
            let seeds = &[PREFIX.as_bytes(),
                ctx.accounts.pool_account.authority.as_ref(),
                ctx.accounts.pool_account.config.as_ref(),
                &[pool_bump]]; // need this to sign the pda, match the authority

            let cpi_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info().clone(),
                anchor_spl::token::SetAuthority {
                    current_authority: ctx.accounts.pool_account.to_account_info().clone(),
                    account_or_mint: ctx.accounts.unstake_from_account.to_account_info().clone(),
                },
            );
            msg!("Calling the token program to transfer authority from pool to unstaker");
            anchor_spl::token::set_authority(cpi_ctx.with_signer(&[&seeds[..]]),
                                             AccountOwner,
                                             Some(ctx.accounts.staker.key()))?;
        }

        Ok(())
    }

    pub fn claim(ctx: Context<ClaimReward>) -> ProgramResult {
        let pool_account = &mut ctx.accounts.pool_account;
        if pool_account.paused || !pool_account.is_initialized {
            return Err(ErrorCode::PoolPaused.into());
        }

        let user_account = &mut ctx.accounts.user_account;
        let user_opt = Some(user_account);
        update_rewards(
            pool_account,
            user_opt,
        ).unwrap();

        // Transfer rewards from the pool reward vaults to user reward vaults.
        let (_pool_pda, pool_bump) = Pubkey::find_program_address(&[PREFIX.as_bytes(),
            ctx.accounts.pool_account.authority.as_ref(),
            ctx.accounts.pool_account.config.as_ref(),
        ], ctx.program_id);
        let seeds = &[PREFIX.as_bytes(),
            ctx.accounts.pool_account.authority.as_ref(),
            ctx.accounts.pool_account.config.as_ref(),
            &[pool_bump]]; // need this to sign the pda, match the authority

        if ctx.accounts.user_account.reward_earned_pending > 0 {
            let mut reward_amount = ctx.accounts.user_account.reward_earned_pending;
            let vault_balance = ctx.accounts.reward_vault.amount;

            // settle pending reward
            ctx.accounts.user_account.reward_earned_pending = 0;
            ctx.accounts.user_account.reward_earned_claimed = ctx.accounts.user_account.reward_earned_claimed + reward_amount;

            if vault_balance < reward_amount {
                reward_amount = vault_balance;
            }

            if reward_amount > 0 {
                let token_program = ctx.accounts.token_program.clone();
                let token_accounts = anchor_spl::token::Transfer {
                    from: ctx.accounts.reward_vault.to_account_info().clone(),
                    to: ctx
                        .accounts
                        .reward_to_account
                        .to_account_info()
                        .clone(),
                    authority: ctx.accounts.pool_account.to_account_info().clone(),
                };
                let cpi_ctx = CpiContext::new(token_program, token_accounts);
                msg!("Calling the token program to transfer reward {} to the user", reward_amount);
                anchor_spl::token::transfer(
                    cpi_ctx.with_signer(&[&seeds[..]]),
                    reward_amount,
                )?;
            }
        }

        Ok(())
    }

    pub fn close_user(ctx: Context<CloseUser>) -> ProgramResult {
        let pool_account = &mut ctx.accounts.pool_account;
        if pool_account.paused || !pool_account.is_initialized {
            return Err(ErrorCode::PoolPaused.into());
        }

        let now = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
        pool_account.last_update_time = now;
        pool_account.user_count = pool_account.user_count.checked_sub(1).unwrap();

        let user_account = &mut ctx.accounts.user_account;
        let user_opt = Some(user_account);
        update_rewards(
            pool_account,
            user_opt,
        ).unwrap();

        if ctx.accounts.user_account.mint_staked_count > 0 {
            return Err(ErrorCode::StakedMint.into());
        }

        if ctx.accounts.user_account.reward_earned_pending > 0 {
            return Err(ErrorCode::PendingRewards.into());
        }

        // ok
        Ok(())
    }

    pub fn close_pool(ctx: Context<ClosePool>) -> ProgramResult {
        // let pool_account = &mut ctx.accounts.pool_account;

        let (_pool_pda, pool_bump) = Pubkey::find_program_address(&[PREFIX.as_bytes(),
            ctx.accounts.pool_account.authority.as_ref(),
            ctx.accounts.pool_account.config.as_ref(),
        ], ctx.program_id);
        let seeds = &[PREFIX.as_bytes(),
            ctx.accounts.pool_account.authority.as_ref(),
            ctx.accounts.pool_account.config.as_ref(),
            &[pool_bump]]; // need this to sign the pda, match the authority

        //close reward vault
        let token_program = ctx.accounts.token_program.clone();
        let token_accounts = anchor_spl::token::Transfer {
            from: ctx.accounts.reward_vault.to_account_info().clone(),
            to: ctx.accounts.reward_refundee.to_account_info().clone(),
            authority: ctx.accounts.pool_account.to_account_info().clone(),
        };
        let cpi_ctx = CpiContext::new(token_program, token_accounts);
        msg!("Calling the token program to refund reward");
        anchor_spl::token::transfer(
            cpi_ctx.with_signer(&[&seeds[..]]),
            ctx.accounts.reward_vault.amount,
        )?;

        let token_program = ctx.accounts.token_program.clone();
        let token_accounts = anchor_spl::token::CloseAccount {
            account: ctx.accounts.reward_vault.to_account_info().clone(),
            destination: ctx.accounts.refundee.to_account_info().clone(),
            authority: ctx.accounts.pool_account.to_account_info().clone(),
        };
        let cpi_ctx = CpiContext::new(token_program, token_accounts);
        msg!("Calling the token program to close reward vault");
        anchor_spl::token::close_account(
            cpi_ctx.with_signer(&[&seeds[..]]),
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(pool_bump: u8, uuid: String, num_mint: u32, reward_bump: u8, reward_duration: u64)]
pub struct InitializePool<'info> {
    // The pool authority
    #[account(mut, signer)]
    authority: AccountInfo<'info>,

    // The pool account, it holds all necessary info about the pool
    #[account(init,
    seeds = [PREFIX.as_bytes(), authority.key.as_ref(), config.key().as_ref()],
    bump = pool_bump,
    payer = authority,
    space = POOL_SIZE)]
    pool_account: ProgramAccount<'info, Pool>,

    // the config account holds the information of the nft token that can be staked
    // the number of mint to stake is pre-determined when the pool is set up
    #[account(zero)]
    config: ProgramAccount<'info, Config>,

    // reward mint
    reward_mint: AccountInfo<'info>,

    // reward vault that holds the reward mint for distribution
    #[account(init,
    token::mint = reward_mint,
    token::authority = pool_account,
    seeds = [PREFIX.as_bytes(),
    pool_account.key().as_ref(),
    authority.key.as_ref(),
    reward_mint.key.as_ref()],
    bump = reward_bump,
    payer = authority
    )]
    reward_vault: Box<Account<'info, TokenAccount>>,

    // The rent sysvar
    rent: Sysvar<'info, Rent>,

    // The Token Program
    #[account(address = spl_token::id())]
    token_program: AccountInfo<'info>,

    // system program
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddMintAddresses<'info> {
    #[account(mut, signer)]
    authority: AccountInfo<'info>,

    // Pool Account
    #[account(mut,
    constraint = pool_account.is_initialized == true,
    constraint = pool_account.config == * config.to_account_info().key,
    has_one = authority,
    )]
    pool_account: ProgramAccount<'info, Pool>,

    #[account(mut, has_one = authority)]
    config: ProgramAccount<'info, Config>,
}

#[derive(Accounts)]
pub struct Pause<'info> {
    #[account(mut, signer)]
    authority: AccountInfo<'info>,

    // Pool Account
    #[account(mut,
    constraint = pool_account.is_initialized == true,
    )]
    pool_account: ProgramAccount<'info, Pool>,
}

#[derive(Accounts)]
pub struct FunderChange<'info> {
    #[account(mut, signer)]
    authority: AccountInfo<'info>,

    // Pool Account
    #[account(mut,
    constraint = pool_account.is_initialized == true,
    has_one = authority,
    )]
    pool_account: ProgramAccount<'info, Pool>,
}

#[derive(Accounts)]
pub struct Fund<'info> {
    // funder
    // verify in the funders list
    #[account(mut, signer,
    constraint = funder.key() == pool_account.authority || pool_account.funders.iter().any(| x | * x == funder.key()),)]
    funder: AccountInfo<'info>,

    // Pool owner
    authority: AccountInfo<'info>,

    // Pool Account
    // verify pool is not paused
    // verify owner
    // verify token vault
    #[account(mut,
    has_one = authority,
    constraint = pool_account.is_initialized == true,
    constraint = pool_account.paused == false,
    constraint = pool_account.reward_vault == * reward_vault.to_account_info().key,
    constraint = pool_account.config == * config.to_account_info().key,
    )]
    pool_account: ProgramAccount<'info, Pool>,

    // the config account
    #[account(has_one = authority)]
    config: ProgramAccount<'info, Config>,

    #[account(mut)]
    reward_vault: Box<Account<'info, TokenAccount>>,

    // funder vault
    #[account(mut)]
    funder_vault: Account<'info, TokenAccount>,

    // The Token Program
    #[account(address = spl_token::id())]
    token_program: AccountInfo<'info>,

}

#[derive(Accounts)]
#[instruction(user_bump: u8, mint_staked_bump: u8, uuid: String)]
pub struct CreateUser<'info> {
    // user owner
    #[account(mut, signer)]
    user: AccountInfo<'info>,

    // Pool Account
    #[account(mut,
    constraint = pool_account.is_initialized == true,
    constraint = pool_account.paused == false,
    )]
    pool_account: ProgramAccount<'info, Pool>,

    // User account where the user info is stored
    #[account(
    init,
    payer = user,
    seeds = [
    PREFIX_USER.as_bytes(),
    pool_account.to_account_info().key.as_ref(),
    user.key.as_ref(),
    ],
    bump = user_bump,
    space = USER_SIZE)]
    user_account: ProgramAccount<'info, User>,

    // new mint staked account
    #[account(
    init,
    payer = user,
    seeds = [
    PREFIX_MINT.as_bytes(),
    pool_account.to_account_info().key.as_ref(),
    user_account.to_account_info().key.as_ref(),
    uuid.as_bytes(),
    ],
    bump = mint_staked_bump,
    space = MINT_STAKED_SIZE_START)]
    mint_staked: ProgramAccount<'info, MintStaked>,

    // The rent sysvar
    rent: Sysvar<'info, Rent>,

    system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(mint_staked_bump: u8, uuid: String)]
pub struct Stake<'info> {
    #[account(mut, signer)]
    staker: AccountInfo<'info>,

    // Pool Account
    // verify pool is not paused
    // verify owner
    // verify config
    #[account(mut,
    has_one = authority,
    constraint = pool_account.is_initialized == true,
    constraint = pool_account.paused == false,
    constraint = pool_account.config == * config.to_account_info().key,
    )]
    pool_account: ProgramAccount<'info, Pool>,

    // the config account
    #[account(mut, has_one = authority)]
    config: ProgramAccount<'info, Config>,

    // Pool owner
    authority: AccountInfo<'info>,

    // user account
    // verify owner is the signer
    // verify pool is the pool account
    #[account(
    mut,
    constraint = user_account.pool == * pool_account.to_account_info().key,
    constraint = user_account.user == * staker.key,
    constraint = user_account.mint_staked == * current_mint_staked.to_account_info().key,
    )]
    user_account: ProgramAccount<'info, User>,

    // account to stake from
    // since we added the nft addresses in config.mint_addresses, this is to check if the mint address in the stake_from_account in the config.mint_addresses
    // constraint = config.mint_addresses.iter().any(| x | * x == stake_from_account.mint)
    #[account(mut,
    )]
    stake_from_account: Box<Account<'info, TokenAccount>>,

    // new mint staked account to store all the mint staked for the user
    // this will keep the information of the token account that holds the nft as an ownership transfer of the whole account will occur in stake/unstake
    #[account(
    init,
    payer = staker,
    seeds = [
    PREFIX_MINT.as_bytes(),
    pool_account.to_account_info().key.as_ref(),
    user_account.to_account_info().key.as_ref(),
    uuid.as_bytes(),
    ],
    bump = mint_staked_bump,
    space = MINT_STAKED_SIZE_START + 32 * (user_account.mint_staked_count + 1) as usize)]
    mint_staked: ProgramAccount<'info, MintStaked>,

    // existing mint staked account
    #[account(mut,
    constraint = current_mint_staked.pool == * pool_account.to_account_info().key,
    constraint = current_mint_staked.user_account == * user_account.to_account_info().key,
    close = staker,
    )]
    current_mint_staked: ProgramAccount<'info, MintStaked>,

    // The rent sysvar
    rent: Sysvar<'info, Rent>,

    // The Token Program
    #[account(address = spl_token::id())]
    token_program: AccountInfo<'info>,

    // system program
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(mint_staked_bump: u8, uuid: String)]
pub struct Unstake<'info> {
    #[account(mut, signer)]
    staker: AccountInfo<'info>,

    // Pool Account
    // verify pool is not paused
    // verify owner
    // verify config
    #[account(mut,
    has_one = authority,
    constraint = pool_account.is_initialized == true,
    constraint = pool_account.paused == false,
    constraint = pool_account.config == * config.to_account_info().key,
    )]
    pool_account: ProgramAccount<'info, Pool>,

    // the config account
    #[account(mut, has_one = authority)]
    config: ProgramAccount<'info, Config>,

    // Pool owner
    authority: AccountInfo<'info>,

    // user account
    // verify owner is the signer
    // verify pool is the pool account
    #[account(
    mut,
    constraint = user_account.pool == * pool_account.to_account_info().key,
    constraint = user_account.user == * staker.key,
    constraint = user_account.mint_staked == * current_mint_staked.to_account_info().key,
    )]
    user_account: ProgramAccount<'info, User>,

    // The nft token account to unstake
    #[account(mut)]
    unstake_from_account: Box<Account<'info, TokenAccount>>,

    // new mint staked account to store all the mint staked for the user
    #[account(
    init,
    payer = staker,
    seeds = [
    PREFIX_MINT.as_bytes(),
    pool_account.to_account_info().key.as_ref(),
    user_account.to_account_info().key.as_ref(),
    uuid.as_bytes(),
    ],
    bump = mint_staked_bump,
    space = MINT_STAKED_SIZE_START + 32 * (user_account.mint_staked_count + 1) as usize)]
    mint_staked: ProgramAccount<'info, MintStaked>,

    // existing mint staked account
    // verify the unstake token account is in the mint staked
    #[account(mut,
    constraint = current_mint_staked.pool == * pool_account.to_account_info().key,
    constraint = current_mint_staked.user_account == * user_account.to_account_info().key,
    constraint = current_mint_staked.mint_accounts.iter().any(| x | * x == * unstake_from_account.to_account_info().key),
    close = staker,
    )]
    current_mint_staked: ProgramAccount<'info, MintStaked>,

    // The rent sysvar
    rent: Sysvar<'info, Rent>,

    // The Token Program
    #[account(address = spl_token::id())]
    token_program: AccountInfo<'info>,

    // system program
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimReward<'info> {
    #[account(mut, signer)]
    user: AccountInfo<'info>,

    // Pool Account
    // verify pool is not paused
    // verify owner
    // verify config
    #[account(mut,
    has_one = authority,
    constraint = pool_account.is_initialized == true,
    constraint = pool_account.paused == false,
    )]
    pool_account: ProgramAccount<'info, Pool>,

    // Pool owner
    authority: AccountInfo<'info>,

    #[account(mut)]
    reward_vault: Box<Account<'info, TokenAccount>>,

    // user account
    // verify owner is the signer
    // verify pool is the pool account
    #[account(
    mut,
    constraint = user_account.pool == * pool_account.to_account_info().key,
    constraint = user_account.user == * user.key,
    )]
    user_account: ProgramAccount<'info, User>,

    // send reward to user reward vault
    #[account(mut)]
    reward_to_account: Box<Account<'info, TokenAccount>>,

    // The Token Program
    #[account(address = spl_token::id())]
    token_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CloseUser<'info> {
    // user owner
    #[account(mut, signer)]
    user: AccountInfo<'info>,

    // Pool Account
    #[account(mut,
    constraint = pool_account.is_initialized == true,
    constraint = pool_account.paused == false,
    )]
    pool_account: ProgramAccount<'info, Pool>,

    // user account
    // user has to unstake everything and claim everything (check in process) before close account
    #[account(
    mut,
    constraint = user_account.pool == * pool_account.to_account_info().key,
    constraint = user_account.user == * user.key,
    close = user,
    )]
    user_account: ProgramAccount<'info, User>,
}

#[derive(Accounts)]
pub struct ClosePool<'info> {
    // authority
    #[account(mut, signer)]
    authority: AccountInfo<'info>,

    // Pool Account
    #[account(mut,
    close = refundee,
    constraint = pool_account.is_initialized == true,
    constraint = pool_account.paused == false,
    constraint = pool_account.authority == * authority.key,
    constraint = pool_account.reward_vault == * reward_vault.to_account_info().key,
    constraint = pool_account.reward_duration_end > 0,
    constraint = pool_account.reward_duration_end < clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap(),
    constraint = pool_account.token_stake_count == 0,
    constraint = pool_account.user_count == 0,
    )]
    pool_account: ProgramAccount<'info, Pool>,

    // the config account
    #[account(mut,
    has_one = authority,
    )]
    config: ProgramAccount<'info, Config>,

    #[account(mut)]
    refundee: AccountInfo<'info>,
    // balance will go to
    #[account(mut)]
    reward_refundee: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    reward_vault: Box<Account<'info, TokenAccount>>,

    // The Token Program
    #[account(address = spl_token::id())]
    token_program: AccountInfo<'info>,

}


pub const POOL_SIZE: usize = 8 + // discriminator
    1 + // is_initialized
    32 + // authority
    1 + // paused
    32 + // config
    32 + // reward_mint
    32 + // reward_vault
    8 + // last_update_time
    16 + // reward_per_token
    8 + // reward_duration
    8 + // reward_duration_end
    4 + // token_stake_count
    4 + // user_count
    4 + 32 * 5; // funders

#[account]
#[derive(Default)]
pub struct Pool {
    // 1
    pub is_initialized: bool,
    /// authority (owner) pubkey
    pub authority: Pubkey,
    /// Paused state of the program
    pub paused: bool,
    /// Config Account that stores all the nft token that can be staked
    pub config: Pubkey,
    /// Mint of the reward token.
    pub reward_mint: Pubkey,
    /// Vault to store reward tokens.
    pub reward_vault: Pubkey,
    /// The last time reward states were updated.
    pub last_update_time: u64,
    /// Reward per token per time unit
    pub reward_rate_per_token: u128,
    /// Reward duration
    pub reward_duration: u64,
    /// Reward duration end
    pub reward_duration_end: u64,
    /// Tokens Staked
    pub token_stake_count: u32,
    /// User created
    pub user_count: u32,
    /// authorized funders
    pub funders: [Pubkey; 5],
}


pub const CONFIG_SIZE_START: usize = 8 + // discriminator
    32 + // authority
    4 + 6 + // uuid + u32 le
    4; // num_mint

#[account]
#[derive(Default)]
pub struct Config {
    /// authority (owner) pubkey
    pub authority: Pubkey,
    /// uuid
    pub uuid: String,
    /// number of token addresses that can be staked
    pub num_mint: u32,
}

pub const USER_SIZE: usize = 8 + // discriminator
    32 + // pool
    32 + // user
    8 + // reward_per_token_complete
    8 + // reward_per_token_pending
    4 + // mint_staked_count
    4 + 6 + // uuid + u32 le
    32 +  // mint_staked
    8; //last update time

// 32 + 32 + 128 + 64 + 32
#[account]
#[derive(Default)]
pub struct User {
    /// Pool the this user belongs to.
    pub pool: Pubkey,
    /// The user
    pub user: Pubkey,
    /// The total amount of reward claimed
    pub reward_earned_claimed: u64,
    /// The total amount of reward pending
    pub reward_earned_pending: u64,
    /// mint staked count
    pub mint_staked_count: u32,
    /// uuid for generating the mint_staked program account for this user
    pub uuid: String,
    /// The mint_staked, hold information all the token account addresses that hold the mint
    pub mint_staked: Pubkey,
    //last update time for stake/unstake
    pub last_update_time: u64,
}

pub const MINT_STAKED_SIZE_START: usize = 8 + // discriminator
    32 + // pool
    32 + // user_account
    4; // u32 len for Vec<Pubkey>

#[account]
#[derive(Default)]
pub struct MintStaked {
    /// Pool
    pub pool: Pubkey,
    /// User account
    pub user_account: Pubkey,
    /// mint addresses
    pub mint_accounts: Vec<Pubkey>,  // theroctically account can hold (10,000,000 - 32 - 32)/32 = 312_497 mint addresses
}

#[error]
pub enum ErrorCode {
    #[msg("Insufficient tokens to stake.")]
    InsufficientTokenStake,
    #[msg("Insufficient funds to stake.")]
    InsufficientFundStake,
    #[msg("Insufficient funds to unstake.")]
    InsufficientFundUnstake,
    #[msg("Amount must be greater than zero.")]
    AmountMustBeGreaterThanZero,
    #[msg("Reward B cannot be funded - pool is single stake.")]
    SingleStakeTokenBCannotBeFunded,
    #[msg("Pool is paused or is not initialized.")]
    PoolPaused,
    #[msg("User has staked mint")]
    StakedMint,
    #[msg("User has pending rewards.")]
    PendingRewards,
    #[msg("Duration cannot be shorter than one day.")]
    DurationTooShort,
    #[msg("Provided funder is already authorized to fund.")]
    FunderAlreadyAuthorized,
    #[msg("Maximum funders already authorized.")]
    MaxFunders,
    #[msg("Cannot deauthorize the primary pool authority.")]
    CannotDeauthorizePoolAuthority,
    #[msg("Authority not found for deauthorization.")]
    CannotDeauthorizeMissingAuthority,
    #[msg("Index greater than length!")]
    IndexGreaterThanLength,
    #[msg("Numerical overflow error!")]
    NumericalOverflowError,
    #[msg("Mint address is not stakable!")]
    InvalidMint,
}