use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, transfer, Mint, TokenAccount};

// This is your program's public key and it will update
// automatically when you build the project.
declare_id!("AmB36oDgbcTtLp7x5E5hqMaj6ivE8mHTG9vsRepY9p9h");

#[program]
mod poer3_layer2_staking {
    use super::*;
    pub fn initialize(ctx: Context<Initialize>, owner: Pubkey) -> Result<()> {
        let pool_info = &mut ctx.accounts.pool_info;
        pool_info.owner = owner;
        pool_info.is_paused = false;
        pool_info.total_mint_reward = 0;
        pool_info.total_eth_mint_reward = 0;
        pool_info.fee_per_thousand = 0;
        pool_info.total_staking = 0;
        pool_info.reward_threshold = 50000 * 10 ^ 9;
        ok(())
    }

    //deposit

    pub fn deposit(ctx: Context<Deposit>, pid: u64, amount: u64) -> Result<()> {
        let pool_info = &mut ctx.accounts.pool_info;
        let user_info = &mut ctx.accounts.user_info;

        //Perform harvest
        Self::harvest(ctx.accounts, pid, ctx.accounts.user.key())?;

        //Deduct fee

        let fee_amount = amount * ctx.amounts.fee_per_thousand / 1000;
        let amount_after_fee = amount - fee_amount - fee_amount;

        //Transfer tokens
        token::transfer(ctx.accounts.info_transfer_to_fee_context(), fee_amount)?;
        token::transfer(
            ctx.accounts.info_transfer_to_pool_context(),
            amount_after_fee,
        )?;

        //Update state
        user_info.amount += amount_after_fee;
        pool_info.amount += amount_after_fee;
        user_info.deposit_time = Clock::get()?.unix_timestamp;

        Ok(())
    }

    //withraw

    pub fn withdraw(ctx: Context<Withdraw>, pid: u64, amount: u64) -> Result<()> {
        let user_info = &mut ctx.accounts.user_info;
        let pool_info = &mut ctx.accounts.pool_info;

        // Check withdrawal conditions
        require!(
            can_withdraw(user_info, pool_info, amount)?,
            CustomError::WithdrawalConditionsNotMet
        );

        // Perform harvest
        Self::harvest(ctx.accounts, pid, ctx.accounts.user.key())?;

        // Update state
        user_info.amount -= amount;
        pool_info.amount -= amount;

        // Transfer tokens
        token::transfer(ctx.accounts.into_transfer_context(), amount)?;

        Ok(())
    }
    fn can_withdraw(u_info: &UserInfo, pool_info: &PoolInfo, amount: u64) -> Result<bool> {
        if u_info.amount < amount {
            return Ok(false);
        }
        let current_timestamp = Clock::get()?.unix_timestamp as u64;

        if pool_info.lock_period > 0 {
            let time_since_deposit = current_timestamp - u_info.deposit_time;
            let in_lock_period = time_since_deposit < pool_info.lock_period;
            let not_in_unlock_period =
                pool_info.unlock_period > 0 && (time_since_deposit % pool_info.lock_period);

            if in_lock_period || not_in_unlock_period {
                return Ok((false));
            }
            Ok(true)
        }
    }
    pub fn emergency_withdraw(ctx: Context<EmergencyWithdraw>, pid: u64) -> Result<()> {
        let user_info = &mut ctx.accounts.user_info;
        let pool_info = &mut ctx.accounts.pool_info;

        //check if emergency withdraw is allowed
        require!(
            pool_info.lock_period == 0 || pool_info.emergency_enable,
            CustomError::EmergencyWithdrawNotAllowed
        );

        //Update state
        let amount = user_info.amount;
        user_info.amount = 0;
        pool_info.amount -= amount;

        //Transfer tokens
        token::transfer(ctx.accounts.into_transfer_context(), amount)?;

        // emit EmergencyWithdraw{
        //     user: ctx.accounts.user.key(),
        //     pid,
        //     amount,
        // }
        Ok(())
    }
    pub fn set_fee(ctx: Context<SetFee>, fee_per_thousand: u16, fee_account: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;

        // Validate fee per thousand
        require!(fee_per_thousand <= 100, CustomError::InvalidFeePerThousand);

        // Update config
        config.fee_per_thousand = fee_per_thousand;
        config.fee_account = fee_account;

        // emit SetFee {
        //     fee_per_thousand,
        //     fee_account,
        // };

        Ok(())
    }
    pub fn set_reward_threshold(
        ctx: Context<SetRewardThreshold>,
        reward_threshold: u64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.reward_threshold = reward_threshold;

        // emit SetRewardThreshold {
        //     reward_threshold,
        // };

        Ok(())
    }
    pub fn set_is_paused(ctx: Context<SetIsPaused>, is_paused: bool) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.is_paused = is_paused;

        // emit SetIsPaused {
        //     is_paused,
        // };

        Ok(())
    }
    pub fn set_vault_contract(
        ctx: Context<SetVaultContract>,
        vault_contract: Pubkey,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.vault_contract = vault_contract;

        // emit SetVaultContract {
        //     vault_contract,
        // };

        Ok(())
    }
    pub fn pool_length(ctx: Context<PoolLength>) -> Result<()> {
        let config = &ctx.accounts.config;
        Ok(config.pool_count as u64)
    }
    pub fn add_pool(
        ctx: Context<AddPool>,
        reward_per_block: u64,
        lock_period: u64,
        unlock_period: u64,
        emergency_enable: bool,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        let pool_info = &mut ctx.accounts.pool_info;

        // Validate lock and unlock periods
        require!(lock_period >= unlock_period, CustomError::InvalidLockPeriod);

        // Update pool info
        pool_info.lp_token = ctx.accounts.lp_token.key();
        pool_info.reward_per_block = reward_per_block;
        pool_info.lock_period = lock_period;
        pool_info.unlock_period = unlock_period;
        pool_info.emergency_enable = emergency_enable;
        pool_info.amount = 0;

        // Update config
        config.pool_count += 1;

        // emit AddPool {
        //     lp_token: pool_info.lp_token,
        //     reward_per_block,
        //     lock_period,
        //     unlock_period,
        //     emergency_enable,
        // };

        Ok(())
    }

}

impl<'info> Initialize<'info> {
    fn only_owner(&self) -> Result<()> {
        if self.owner.key() != *self.authority.to_amount_info().key {
            return Err(ErrorCode::Unauthorized.info());
        }
        Ok(())
    }
}

impl<'info> Deposit<'info> {
    fn into_transfer_too_fee_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        CpiContext::new(
            self.token_program.to_account_info().close(),
            Transfer {
                from: self.lp_token_account.to_account_info().clone(),
                to: self.fee_account.to_account_info().clone(),
                authority: self.user.to_account_info().clone(),
            },
        )
    }

    fn intro_transfer_to_pool_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        CpiContext::new(
            self.token_program.to_account_info().clone(),
            Transfer {
                from: self.lp_token_account.to_account_info().clone(),
                to: self.pool_info.to_account_info().clone(),
                authority: self.user.to_account_info().clone(),
            },
        )
    }
}

impl<'info> Withdraw<'info> {
    fn into_transfer_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        CpiContext::new(
            sel.token_program.to_account_info.clone(),
            Transfer {
                from: self.lp_token_account.to_account_info().clone(),
                to: self.destination.to_account_info().clone(),
                authority: self.user.to_account_info().clone(),
            },
        )
    }
}

impl<'info> EmergencyWithdraw<'info> {
    fn into_transfer_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        CpiContext::new(
            self.token_program.to_account_info().clone(),
            Transfer {
                from: self.lp_token_account.to_account_info().clone(),
                to: self.destination.to_account_info().clone(),
                authority: self.user.to_account_info().clone(),
            },
        )
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    // We must specify the space in order to initialize an account.
    // First 8 bytes are default account discriminator,
    // next 8 bytes come from NewAccount.data being type u64.
    // (u64 = 64 bits unsigned integer = 8 bytes)
    #[account(init)]
    pub owner: Signer<'info>,
    pub port3_vault: Account<'info, Port3Vault>,
    pub system_program: Program<'info, System>,
    // #[account(mut)]
    // pub signer: Signer<'info>,
    // pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_info: Account<'info, UserInfo>,
    #[account(mut)]
    pub pool_info: Account<'info, PoolInfo>,
    #[account(mut)]
    pub lp_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub fee_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_info: Account<'info, UserInfo>,
    #[account(mut)]
    pub pool_info: Account<'info, PoolInfo>,
    #[account(mut)]
    pub lp_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub fee_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct EmergencyWithdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_info: Account<'info, UserInfo>,
    #[account(mut)]
    pub pool_info: Account<'info, PoolInfo>,
    #[account(mut)]
    pub lp_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub destination: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct SetVaultContract<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub config: Account<'info, Config>,
}
#[derive(Accounts)]
pub struct SetFee<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub config: Account<'info, Config>,
}
#[derive(Accounts)]
pub struct SetRewardThreshold<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub config: Account<'info, Config>,
}
#[derive(Accounts)]
pub struct SetIsPaused<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub config: Account<'info, Config>,
}
#[derive(Accounts)]
pub struct PoolLength<'info> {
    #[account(mut)]
    pub config: Account<'info, Config>,
}
#[derive(Accounts)]
pub struct AddPool<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub config: Account<'info, Config>,
    #[account(init, payer = owner, space = 8 + 8 + 8 + 8 + 1 + 8)]
    pub pool_info: Account<'info, PoolInfo>,
    #[account(mut)]
    pub lp_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
#[account]
pub struct PoolInfo {
    lp_token: Pubkey,
    amount: u64,
    lock_period: u64,
    apy: u64,
    start_time: i64,
    end_time: i64,
    is_open_reward: bool,
    is_reward_et: bool,
    emergency_enable: bool,
}
#[account]
pub struct UserInfo {
    amount: u64,
    reward_claimed: u64,
    deposit_time: i64,
    last_harvest_time: i64,
}
