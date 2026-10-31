//! 积分：日领（PostgreSQL 幂等）/ 广告 / 出稿扣减 / 示意充值。

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub const DAILY_GRANT: i32 = 3;
/// 一次成功的 MVP+ 出稿（去背 + 卡通 + 拼豆量化）只扣这一笔。
/// 失败或取消不会走到扣减，因此也不另做退款。
pub const GENERATE_COST: i32 = 1;
pub const AD_REWARD: i32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct Pack {
    pub code: &'static str,
    pub name: &'static str,
    pub credits: i32,
    pub price_cents: i32,
    pub price_label: &'static str,
    pub blurb: &'static str,
    pub popular: bool,
}

pub const PACKS: &[Pack] = &[
    Pack {
        code: "small",
        name: "小包",
        credits: 10,
        price_cents: 600,
        price_label: "¥6",
        blurb: "约 0.6 元/次出稿",
        popular: false,
    },
    Pack {
        code: "popular",
        name: "推荐",
        credits: 35,
        price_cents: 1800,
        price_label: "¥18",
        blurb: "多送 5 · 示意",
        popular: true,
    },
    Pack {
        code: "large",
        name: "大包",
        credits: 60,
        price_cents: 3000,
        price_label: "¥30",
        blurb: "多送 10 · 示意",
        popular: false,
    },
];

#[derive(Debug, Serialize)]
pub struct CreditsView {
    pub balance: i32,
    pub daily_grant: i32,
    pub generate_cost: i32,
    pub ad_reward: i32,
    pub daily_claimed_today: bool,
}

#[derive(Debug, Serialize)]
pub struct CreditMutation {
    pub balance: i32,
    pub delta: i32,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct LedgerItem {
    pub id: Uuid,
    pub delta: i32,
    pub balance_after: i32,
    pub reason: String,
    pub reason_label: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct OrderView {
    pub order_id: Uuid,
    pub pack_code: String,
    pub credits: i32,
    pub price_cents: i32,
    pub price_label: String,
    pub status: String,
    pub mock: bool,
}

pub fn china_today() -> NaiveDate {
    Utc::now().with_timezone(&china_offset()).date_naive()
}

fn china_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("offset")
}

pub fn reason_label(reason: &str) -> &'static str {
    match reason {
        "daily_claim" => "每日领取",
        "ad_reward" => "激励广告",
        "generate" => "成功出稿",
        "recharge" => "充值到账",
        _ => "积分变动",
    }
}

pub async fn view(state: &AppState, user_id: Uuid) -> AppResult<CreditsView> {
    let balance = balance_of(&state.db, user_id).await?;
    let claimed = claimed_today(&state.db, user_id, china_today()).await?;
    Ok(CreditsView {
        balance,
        daily_grant: DAILY_GRANT,
        generate_cost: GENERATE_COST,
        ad_reward: AD_REWARD,
        daily_claimed_today: claimed,
    })
}

pub async fn daily_claim(state: &AppState, user_id: Uuid) -> AppResult<CreditMutation> {
    let day = china_today();
    let balance = grant_daily_in_db(state, user_id, day).await?;
    Ok(CreditMutation {
        balance,
        delta: DAILY_GRANT,
        reason: "daily_claim".into(),
    })
}

async fn grant_daily_in_db(state: &AppState, user_id: Uuid, day: NaiveDate) -> AppResult<i32> {
    let mut tx = state.db.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO daily_claims (user_id, claim_date) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(day)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(AppError::conflict(
            "already_claimed",
            "今日已领取，明天再来吧",
        ));
    }
    let balance = apply_delta_tx(&mut tx, user_id, DAILY_GRANT, "daily_claim", None).await?;
    tx.commit().await?;
    Ok(balance)
}

pub async fn ad_reward(state: &AppState, user_id: Uuid) -> AppResult<CreditMutation> {
    let mut tx = state.db.begin().await?;
    let balance = apply_delta_tx(&mut tx, user_id, AD_REWARD, "ad_reward", None).await?;
    tx.commit().await?;
    Ok(CreditMutation {
        balance,
        delta: AD_REWARD,
        reason: "ad_reward".into(),
    })
}

pub async fn ledger(pool: &PgPool, user_id: Uuid, limit: i64) -> AppResult<Vec<LedgerItem>> {
    let limit = limit.clamp(1, 50);
    let rows = sqlx::query_as::<_, LedgerRow>(
        r#"
        SELECT id, delta, balance_after, reason, created_at
        FROM credit_ledger
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT $2
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| LedgerItem {
            id: row.id,
            delta: row.delta,
            balance_after: row.balance_after,
            reason_label: reason_label(&row.reason).into(),
            reason: row.reason,
            created_at: row.created_at,
        })
        .collect())
}

pub fn find_pack(code: &str) -> AppResult<&'static Pack> {
    PACKS
        .iter()
        .find(|p| p.code == code)
        .ok_or_else(|| AppError::BadRequest("没有这个充值档位".into()))
}

pub async fn create_order(pool: &PgPool, user_id: Uuid, pack_code: &str) -> AppResult<OrderView> {
    let pack = find_pack(pack_code)?;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO credit_orders (id, user_id, pack_code, credits, price_cents, status)
        VALUES ($1, $2, $3, $4, $5, 'created')
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(pack.code)
    .bind(pack.credits)
    .bind(pack.price_cents)
    .execute(pool)
    .await?;
    Ok(order_view(id, pack, "created"))
}

pub async fn mock_pay(
    state: &AppState,
    user_id: Uuid,
    order_id: Uuid,
) -> AppResult<(OrderView, i32, i32)> {
    let mut tx = state.db.begin().await?;
    let row = sqlx::query_as::<_, OrderRow>(
        r#"
        SELECT id, user_id, pack_code, credits, price_cents, status
        FROM credit_orders
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("订单不存在".into()))?;

    if row.user_id != user_id {
        return Err(AppError::Forbidden("不能支付别人的订单".into()));
    }
    let pack = find_pack(&row.pack_code)?;
    if row.status == "paid" {
        let balance = balance_of_tx(&mut tx, user_id).await?;
        tx.commit().await?;
        return Ok((order_view(row.id, pack, "paid"), balance, 0));
    }

    sqlx::query("UPDATE credit_orders SET status = 'paid', paid_at = now() WHERE id = $1")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    let balance = apply_delta_tx(
        &mut tx,
        user_id,
        row.credits,
        "recharge",
        Some(&order_id.to_string()),
    )
    .await?;
    tx.commit().await?;
    Ok((order_view(row.id, pack, "paid"), balance, row.credits))
}

pub async fn apply_delta_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    delta: i32,
    reason: &str,
    ref_id: Option<&str>,
) -> AppResult<i32> {
    let current = balance_of_tx(tx, user_id).await?;
    if current + delta < 0 {
        return Err(AppError::InsufficientCredits(format!(
            "积分不足：需要 {}，当前 {current}",
            -delta
        )));
    }
    let next = current + delta;
    sqlx::query("UPDATE credit_accounts SET balance = $2, updated_at = now() WHERE user_id = $1")
        .bind(user_id)
        .bind(next)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO credit_ledger (user_id, delta, balance_after, reason, ref_id)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(user_id)
    .bind(delta)
    .bind(next)
    .bind(reason)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(next)
}

async fn balance_of(pool: &PgPool, user_id: Uuid) -> AppResult<i32> {
    let balance: Option<i32> =
        sqlx::query_scalar("SELECT balance FROM credit_accounts WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(balance.unwrap_or(0))
}

async fn balance_of_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
) -> AppResult<i32> {
    let balance: Option<i32> =
        sqlx::query_scalar("SELECT balance FROM credit_accounts WHERE user_id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?;
    balance.ok_or_else(|| AppError::NotFound("积分账户不存在".into()))
}

async fn claimed_today(pool: &PgPool, user_id: Uuid, day: NaiveDate) -> AppResult<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM daily_claims WHERE user_id = $1 AND claim_date = $2)",
    )
    .bind(user_id)
    .bind(day)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

fn order_view(id: Uuid, pack: &Pack, status: &str) -> OrderView {
    OrderView {
        order_id: id,
        pack_code: pack.code.into(),
        credits: pack.credits,
        price_cents: pack.price_cents,
        price_label: pack.price_label.into(),
        status: status.into(),
        mock: true,
    }
}

#[derive(sqlx::FromRow)]
struct LedgerRow {
    id: Uuid,
    delta: i32,
    balance_after: i32,
    reason: String,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct OrderRow {
    id: Uuid,
    user_id: Uuid,
    pack_code: String,
    credits: i32,
    #[allow(dead_code)]
    price_cents: i32,
    status: String,
}

/// 锁内读取余额。供出稿事务复用。
pub async fn lock_balance(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
) -> AppResult<i32> {
    balance_of_tx(tx, user_id).await
}
