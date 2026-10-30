//! 积分规则
//!
//! - 每日登录送 3（daily claim）
//! - 出稿扣 1
//! - 激励广告 +1
//! - 充值档位见 docs/api.md（占位）

pub const DAILY_GRANT: i32 = 3;
pub const GENERATE_COST: i32 = 1;
pub const AD_REWARD: i32 = 1;

pub fn apply_daily_claim(balance: i32) -> i32 {
    balance + DAILY_GRANT
}

pub fn apply_ad_reward(balance: i32) -> i32 {
    balance + AD_REWARD
}

pub fn try_spend_generate(balance: i32) -> Result<i32, String> {
    if balance < GENERATE_COST {
        return Err(format!(
            "积分不足：需要 {GENERATE_COST}，当前 {balance}"
        ));
    }
    Ok(balance - GENERATE_COST)
}
