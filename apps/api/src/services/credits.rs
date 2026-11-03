//! 积分：日领（PostgreSQL 幂等）/ 广告 / 出稿扣减 / 充值。
//!
//! 充值入账只有三条路径：未配置支付时的开发示意回调、验签通过的微信支付结果、
//! 以及虚拟支付的发货推送或查单。三条路径都锁订单，只在 `created → paid` 时加一次积分。

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::services::session_seal;
use crate::services::virtual_pay::{
    self, DeliverPush, QueryOutcome, VirtualMode, VirtualPayParams,
};
use crate::services::wechat_pay::{self, OpenedNotify, PayMode, VerifiedTransaction};
use crate::state::AppState;

pub const DAILY_GRANT: i32 = 3;
/// 一次成功的 MVP+ 出稿（Seedream 出稿 + 拼豆量化）只扣这一笔。
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
        blurb: "多送 5",
        popular: true,
    },
    Pack {
        code: "large",
        name: "大包",
        credits: 60,
        price_cents: 3000,
        price_label: "¥30",
        blurb: "多送 10",
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
    pub out_trade_no: String,
    pub pack_code: String,
    pub credits: i32,
    /// 标价，单位分。与 `amount_fen`、微信支付 `amount.total` 一致。
    pub price_cents: i32,
    pub amount_fen: i32,
    pub price_label: String,
    pub status: String,
    /// 开发示意单。正式支付为 false，此时不会通过 mock-callback 入账。
    pub mock: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pay_params: Option<wechat_pay::JsapiPayParams>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_payment: Option<VirtualPayParams>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<i32>,
}

enum Checkout {
    Mock,
    Misconfigured(String),
    Virtual(virtual_pay::VirtualCreds),
    Wechat(wechat_pay::PayCredentials),
}

fn checkout_mode(config: &Config) -> Checkout {
    match virtual_pay::virtual_mode(config) {
        VirtualMode::Live(creds) => Checkout::Virtual(creds),
        VirtualMode::Misconfigured(reason) => Checkout::Misconfigured(reason),
        VirtualMode::Unset => match wechat_pay::pay_mode(config) {
            PayMode::Mock => Checkout::Mock,
            PayMode::Misconfigured(reason) => Checkout::Misconfigured(reason),
            PayMode::Live(creds) => Checkout::Wechat(creds),
        },
    }
}

pub fn describe_checkout(config: &Config) -> String {
    match checkout_mode(config) {
        Checkout::Mock => "充值未配置虚拟支付或商户参数，走开发示意（mock-callback）".into(),
        Checkout::Misconfigured(reason) => format!("充值配置不完整，示意到账已关闭：{reason}"),
        Checkout::Virtual(creds) => format!(
            "虚拟支付已启用（env={}，发货通知 {}）",
            creds.env,
            virtual_pay::notify_url(config)
        ),
        Checkout::Wechat(_) => wechat_pay::describe_pay_mode(config),
    }
}

pub struct PackListMeta {
    pub mock: bool,
    pub pay_mode: &'static str,
    pub note: &'static str,
}

pub fn pack_list_meta(state: &AppState) -> PackListMeta {
    match checkout_mode(&state.config) {
        Checkout::Mock => PackListMeta {
            mock: true,
            pay_mode: "mock",
            note: "未配置虚拟支付或微信支付。mock-callback 直接到账，不会产生真实扣款",
        },
        Checkout::Virtual(_) => PackListMeta {
            mock: false,
            pay_mode: "virtual",
            note: "微信虚拟支付。发货通知或查单确认后到账，未支付不加积分",
        },
        Checkout::Wechat(_) => PackListMeta {
            mock: false,
            pay_mode: "wechat",
            note: "微信支付。验签成功后到账，未支付不加积分",
        },
        Checkout::Misconfigured(_) => PackListMeta {
            mock: false,
            pay_mode: "misconfigured",
            note: "支付配置不完整，示意到账已关闭",
        },
    }
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

pub async fn create_order(
    state: &AppState,
    user_id: Uuid,
    pack_code: &str,
) -> AppResult<OrderView> {
    let pack = find_pack(pack_code)?;
    match checkout_mode(&state.config) {
        Checkout::Misconfigured(reason) => {
            tracing::warn!(%reason, "refuse credit order because pay is misconfigured");
            Err(AppError::ServiceUnavailable(format!(
                "支付配置不完整：{reason}"
            )))
        }
        Checkout::Mock => {
            let row = insert_created(state, user_id, pack, "mock").await?;
            Ok(order_view(&row, true, None, None, None))
        }
        Checkout::Wechat(creds) => create_wechat_order(state, user_id, pack, creds).await,
        Checkout::Virtual(creds) => create_virtual_order(state, user_id, pack, creds).await,
    }
}

async fn create_wechat_order(
    state: &AppState,
    user_id: Uuid,
    pack: &Pack,
    creds: wechat_pay::PayCredentials,
) -> AppResult<OrderView> {
    let row = insert_created(state, user_id, pack, "wechat").await?;
    let openid = row
        .payer_openid
        .clone()
        .ok_or_else(|| AppError::BadRequest("缺少付款人 openid".into()))?;
    let description = format!("拼豆积分-{}", pack.name);
    let prepay_id = wechat_pay::jsapi_prepay(
        state,
        &creds,
        &description,
        &row.out_trade_no,
        pack.price_cents,
        &openid,
    )
    .await?;
    if let Err(err) = sqlx::query("UPDATE credit_orders SET prepay_id = $2 WHERE id = $1")
        .bind(row.id)
        .bind(&prepay_id)
        .execute(&state.db)
        .await
    {
        tracing::error!(error = %err, out_trade_no = %row.out_trade_no, "save prepay_id failed");
    }
    let params = wechat_pay::mini_program_pay_params(&creds, &prepay_id)?;
    Ok(order_view(&row, false, Some(params), None, None))
}

async fn create_virtual_order(
    state: &AppState,
    user_id: Uuid,
    pack: &Pack,
    creds: virtual_pay::VirtualCreds,
) -> AppResult<OrderView> {
    let session_key = load_session_key(state, user_id).await?;
    let product_id = creds
        .product_id(pack.code)
        .map_err(|err| AppError::ServiceUnavailable(err))?;
    let row = insert_created(state, user_id, pack, "virtual").await?;
    let attach = virtual_pay::attach_for(pack.code, &row.out_trade_no);
    let sign_data = virtual_pay::sign_data_json(
        &creds,
        &product_id,
        pack.price_cents,
        &row.out_trade_no,
        &attach,
    )?;
    let pay_sig = virtual_pay::pay_sig(
        &creds.app_key,
        virtual_pay::REQUEST_VIRTUAL_PAYMENT,
        &sign_data,
    )
    .map_err(|_| AppError::ServiceUnavailable("虚拟支付签名失败".into()))?;
    let signature = virtual_pay::user_signature(&session_key, &sign_data)
        .map_err(|_| AppError::ServiceUnavailable("虚拟支付签名失败".into()))?;
    drop(session_key);
    Ok(order_view(
        &row,
        false,
        None,
        Some(VirtualPayParams {
            sign_data,
            pay_sig,
            signature,
            mode: "short_series_goods".into(),
        }),
        None,
    ))
}

async fn insert_created(
    state: &AppState,
    user_id: Uuid,
    pack: &Pack,
    channel: &str,
) -> AppResult<OrderRow> {
    let openid = user_openid(&state.db, user_id).await?;
    let id = Uuid::new_v4();
    let out_trade_no = id.simple().to_string();
    if channel == "virtual" && !virtual_pay::valid_out_trade_no(&out_trade_no) {
        return Err(AppError::BadRequest("商户订单号无效".into()));
    }
    sqlx::query(
        r#"
        INSERT INTO credit_orders (
            id, user_id, pack_code, credits, price_cents, amount_fen, status,
            out_trade_no, payer_openid, pay_channel
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'created', $7, $8, $9)
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(pack.code)
    .bind(pack.credits)
    .bind(pack.price_cents)
    .bind(pack.price_cents)
    .bind(&out_trade_no)
    .bind(&openid)
    .bind(channel)
    .execute(&state.db)
    .await?;
    order_by_id(&state.db, id).await
}

pub async fn get_order(state: &AppState, user_id: Uuid, order_id: Uuid) -> AppResult<OrderView> {
    let row = order_by_id(&state.db, order_id).await?;
    if row.user_id != user_id {
        return Err(AppError::Forbidden("不能查看别人的订单".into()));
    }
    if row.status != "paid" {
        match row.pay_channel.as_str() {
            "wechat" => try_reconcile_wechat(state, &row).await,
            "virtual" => try_reconcile_virtual(state, &row).await,
            _ => {}
        }
    }
    let row = order_by_id(&state.db, order_id).await?;
    let balance = balance_of(&state.db, user_id).await?;
    let mock = matches!(checkout_mode(&state.config), Checkout::Mock) && row.pay_channel == "mock";
    Ok(order_view(&row, mock, None, None, Some(balance)))
}

pub async fn mock_pay(
    state: &AppState,
    user_id: Uuid,
    order_id: Uuid,
) -> AppResult<(OrderView, i32, i32)> {
    if !matches!(checkout_mode(&state.config), Checkout::Mock) {
        return Err(AppError::Forbidden("开发示意到账已关闭".into()));
    }
    let mut tx = state.db.begin().await?;
    let row = lock_order_by_id(&mut tx, order_id).await?;
    if row.user_id != user_id {
        return Err(AppError::Forbidden("不能支付别人的订单".into()));
    }
    if row.pay_channel != "mock" {
        return Err(AppError::Forbidden("这笔订单不能示意到账".into()));
    }
    let (balance, delta) = credit_locked_order(&mut tx, &row, None).await?;
    tx.commit().await?;
    let row = order_by_id(&state.db, order_id).await?;
    Ok((
        order_view(&row, true, None, None, Some(balance)),
        balance,
        delta,
    ))
}

pub async fn apply_wechat_payment(state: &AppState, notice: &VerifiedTransaction) -> AppResult<()> {
    match apply_wechat_payment_once(state, notice).await {
        Err(AppError::Conflict {
            code: "duplicate_payment",
            ..
        }) => confirm_already_paid(state, notice).await,
        other => other,
    }
}

async fn apply_wechat_payment_once(
    state: &AppState,
    notice: &VerifiedTransaction,
) -> AppResult<()> {
    let PayMode::Live(creds) = wechat_pay::pay_mode(&state.config) else {
        return Err(AppError::Forbidden("微信支付未启用".into()));
    };
    if notice.appid != creds.app_id || notice.mchid != creds.mch_id {
        return Err(AppError::BadRequest("支付单的 appid 或商户号不匹配".into()));
    }
    if notice.trade_state != "SUCCESS" || notice.transaction_id.is_empty() {
        return Err(AppError::BadRequest("支付尚未成功".into()));
    }

    let mut tx = state.db.begin().await?;
    let row = lock_order_by_trade_no(&mut tx, &notice.out_trade_no).await?;
    if row.pay_channel != "wechat" {
        return Err(AppError::BadRequest("订单不是微信支付单".into()));
    }
    if row.amount_fen as i64 != notice.amount_total {
        return Err(AppError::BadRequest("支付金额与订单不一致".into()));
    }
    if row.payer_openid.as_deref() != Some(notice.payer_openid.as_str()) {
        return Err(AppError::BadRequest("支付用户与订单不一致".into()));
    }
    if row.status == "paid" {
        if let Some(existing) = &row.transaction_id {
            if existing != &notice.transaction_id {
                return Err(AppError::conflict(
                    "transaction_mismatch",
                    "订单已绑定另一笔微信支付",
                ));
            }
        }
        tx.commit().await?;
        return Ok(());
    }
    credit_locked_order(&mut tx, &row, Some(&notice.transaction_id)).await?;
    tx.commit().await?;
    tracing::info!(
        user_id = %row.user_id,
        order_id = %row.id,
        out_trade_no = %row.out_trade_no,
        credits = row.credits,
        "wechat pay credited"
    );
    Ok(())
}

async fn confirm_already_paid(state: &AppState, notice: &VerifiedTransaction) -> AppResult<()> {
    let sql = format!("SELECT {ORDER_COLUMNS} FROM credit_orders WHERE out_trade_no = $1");
    let row = sqlx::query_as::<_, OrderRow>(&sql)
        .bind(&notice.out_trade_no)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("订单不存在".into()))?;
    if row.status == "paid" && row.transaction_id.as_deref() == Some(notice.transaction_id.as_str())
    {
        return Ok(());
    }
    Err(AppError::conflict("duplicate_payment", "重复的支付通知"))
}

/// 回调验签在路由里做完。这里只入账。非支付成功通知直接忽略。
pub async fn accept_opened_notify(state: &AppState, opened: OpenedNotify) -> AppResult<()> {
    match opened {
        OpenedNotify::Ignored => Ok(()),
        OpenedNotify::Paid(tx) => apply_wechat_payment(state, &tx).await,
    }
}

async fn try_reconcile_wechat(state: &AppState, row: &OrderRow) {
    let PayMode::Live(creds) = wechat_pay::pay_mode(&state.config) else {
        return;
    };
    if row.pay_channel != "wechat" || row.status == "paid" {
        return;
    }
    match wechat_pay::query_paid_transaction(state, &creds, &row.out_trade_no).await {
        Ok(Some(tx)) => {
            if let Err(err) = apply_wechat_payment(state, &tx).await {
                tracing::warn!(error = %err, out_trade_no = %row.out_trade_no, "reconcile wechat order failed");
            }
        }
        Ok(None) => {}
        Err(err) => {
            tracing::warn!(error = %err, out_trade_no = %row.out_trade_no, "query wechat order failed");
        }
    }
}

async fn try_reconcile_virtual(state: &AppState, row: &OrderRow) {
    let Checkout::Virtual(creds) = checkout_mode(&state.config) else {
        return;
    };
    if row.pay_channel != "virtual" || row.status == "paid" {
        return;
    }
    let Some(openid) = row.payer_openid.clone() else {
        return;
    };
    match virtual_pay::query_order(state, &creds, &openid, &row.out_trade_no).await {
        Ok(QueryOutcome::Paid(facts)) => {
            if fees_conflict(row.amount_fen as i64, facts.order_fee, facts.paid_fee) {
                tracing::warn!(out_trade_no = %row.out_trade_no, "virtual query amount mismatch");
                return;
            }
            if let Err(err) = apply_virtual_grant(
                state,
                &VirtualGrant {
                    out_trade_no: row.out_trade_no.clone(),
                    transaction_id: facts.transaction_id,
                    payer_openid: openid,
                    amount_fen: row.amount_fen as i64,
                },
            )
            .await
            {
                tracing::warn!(error = %err, out_trade_no = %row.out_trade_no, "reconcile virtual order failed");
                return;
            }
            if let Err(err) = virtual_pay::notify_delivered(
                state,
                &creds,
                &row.out_trade_no,
                facts.wx_order_id.as_deref(),
            )
            .await
            {
                tracing::warn!(error = %err, out_trade_no = %row.out_trade_no, "notify virtual delivery failed");
            }
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(error = %err, out_trade_no = %row.out_trade_no, "query virtual order failed");
        }
    }
}

/// 道具发货推送。验签由路由完成。查单确认优先；查单不可用时，已验签且字段吻合才入账。
pub async fn accept_virtual_deliver(
    state: &AppState,
    push: &DeliverPush,
    signature_verified: bool,
) -> AppResult<()> {
    let Checkout::Virtual(creds) = checkout_mode(&state.config) else {
        return Err(AppError::Forbidden("虚拟支付未启用".into()));
    };
    if !virtual_pay::valid_out_trade_no(&push.out_trade_no) {
        return Err(AppError::BadRequest("商户订单号无效".into()));
    }
    if push.env != creds.env {
        return Err(AppError::BadRequest("支付环境与订单不一致".into()));
    }
    let row = order_by_trade_no(&state.db, &push.out_trade_no).await?;
    if row.pay_channel != "virtual" {
        return Err(AppError::BadRequest("订单不是虚拟支付单".into()));
    }
    if row.payer_openid.as_deref() != Some(push.openid.as_str()) {
        return Err(AppError::BadRequest("支付用户与订单不一致".into()));
    }
    let product_id = creds
        .product_id(&row.pack_code)
        .map_err(|err| AppError::BadRequest(err))?;
    if push.product_id != product_id || push.quantity != 1 {
        return Err(AppError::BadRequest("道具与订单不一致".into()));
    }
    if fees_conflict(
        row.amount_fen as i64,
        Some(push.orig_price),
        Some(push.actual_price),
    ) {
        return Err(AppError::BadRequest("支付金额与订单不一致".into()));
    }
    let expected_attach = virtual_pay::attach_for(&row.pack_code, &row.out_trade_no);
    if !push.attach.is_empty() && push.attach != expected_attach {
        return Err(AppError::BadRequest("透传数据与订单不一致".into()));
    }
    if row.status == "paid" {
        return Ok(());
    }
    let verified_push = signature_verified;
    match virtual_pay::query_order(state, &creds, &push.openid, &push.out_trade_no).await {
        Ok(QueryOutcome::Paid(facts)) => {
            if fees_conflict(row.amount_fen as i64, facts.order_fee, facts.paid_fee) {
                return Err(AppError::BadRequest("支付金额与订单不一致".into()));
            }
            apply_virtual_grant(
                state,
                &VirtualGrant {
                    out_trade_no: row.out_trade_no.clone(),
                    transaction_id: facts.transaction_id,
                    payer_openid: push.openid.clone(),
                    amount_fen: row.amount_fen as i64,
                },
            )
            .await
        }
        Ok(QueryOutcome::ClosedOrRefunded) => Ok(()),
        Ok(QueryOutcome::NotReady) | Err(_) if verified_push => {
            apply_virtual_grant(
                state,
                &VirtualGrant {
                    out_trade_no: row.out_trade_no,
                    transaction_id: push.transaction_id.clone(),
                    payer_openid: push.openid.clone(),
                    amount_fen: row.amount_fen as i64,
                },
            )
            .await
        }
        Ok(QueryOutcome::NotReady) => Err(AppError::ServiceUnavailable("支付结果未确认".into())),
        Err(err) => Err(err),
    }
}

struct VirtualGrant {
    out_trade_no: String,
    transaction_id: String,
    payer_openid: String,
    amount_fen: i64,
}

fn fees_conflict(expected: i64, left: Option<i64>, right: Option<i64>) -> bool {
    let mut saw = false;
    for fee in [left, right] {
        if let Some(fee) = fee {
            if fee > 0 {
                saw = true;
                if fee != expected {
                    return true;
                }
            }
        }
    }
    !saw
}

async fn apply_virtual_grant(state: &AppState, grant: &VirtualGrant) -> AppResult<()> {
    match apply_virtual_grant_once(state, grant).await {
        Err(AppError::Conflict {
            code: "duplicate_payment",
            ..
        }) => {
            let row = order_by_trade_no(&state.db, &grant.out_trade_no).await?;
            if row.status == "paid" {
                Ok(())
            } else {
                Err(AppError::conflict("duplicate_payment", "重复的支付通知"))
            }
        }
        other => other,
    }
}

async fn apply_virtual_grant_once(state: &AppState, grant: &VirtualGrant) -> AppResult<()> {
    if !matches!(checkout_mode(&state.config), Checkout::Virtual(_)) {
        return Err(AppError::Forbidden("虚拟支付未启用".into()));
    }
    if grant.transaction_id.is_empty() {
        return Err(AppError::BadRequest("缺少支付单号".into()));
    }
    let mut tx = state.db.begin().await?;
    let row = lock_order_by_trade_no(&mut tx, &grant.out_trade_no).await?;
    if row.pay_channel != "virtual" {
        return Err(AppError::BadRequest("订单不是虚拟支付单".into()));
    }
    if row.amount_fen as i64 != grant.amount_fen {
        return Err(AppError::BadRequest("支付金额与订单不一致".into()));
    }
    if row.payer_openid.as_deref() != Some(grant.payer_openid.as_str()) {
        return Err(AppError::BadRequest("支付用户与订单不一致".into()));
    }
    if row.status == "paid" {
        if let Some(existing) = &row.transaction_id {
            let same = existing == &grant.transaction_id
                || existing.starts_with("xpay:")
                || grant.transaction_id.starts_with("xpay:");
            if !same {
                return Err(AppError::conflict(
                    "transaction_mismatch",
                    "订单已绑定另一笔支付",
                ));
            }
        }
        tx.commit().await?;
        return Ok(());
    }
    credit_locked_order(&mut tx, &row, Some(&grant.transaction_id)).await?;
    tx.commit().await?;
    tracing::info!(
        user_id = %row.user_id,
        order_id = %row.id,
        out_trade_no = %row.out_trade_no,
        credits = row.credits,
        "virtual pay credited"
    );
    Ok(())
}

async fn load_session_key(state: &AppState, user_id: Uuid) -> AppResult<String> {
    let stored: Option<Option<String>> =
        sqlx::query_scalar("SELECT wx_session_key_enc FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
    let Some(stored) = stored.flatten().filter(|value| !value.trim().is_empty()) else {
        return Err(AppError::Unauthorized(
            "登录已过期，请重新登录后再支付".into(),
        ));
    };
    session_seal::open(&state.config.jwt_secret, &stored)
        .map_err(|_| AppError::Unauthorized("登录已过期，请重新登录后再支付".into()))
}

async fn credit_locked_order(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    row: &OrderRow,
    transaction_id: Option<&str>,
) -> AppResult<(i32, i32)> {
    if row.status == "paid" {
        let balance = balance_of_tx(tx, row.user_id).await?;
        return Ok((balance, 0));
    }
    let updated = sqlx::query(
        r#"
        UPDATE credit_orders
        SET status = 'paid', paid_at = now(), transaction_id = $2
        WHERE id = $1 AND status = 'created'
        "#,
    )
    .bind(row.id)
    .bind(transaction_id)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    if updated.rows_affected() == 0 {
        let balance = balance_of_tx(tx, row.user_id).await?;
        return Ok((balance, 0));
    }
    let balance = apply_delta_tx(
        tx,
        row.user_id,
        row.credits,
        "recharge",
        Some(&row.id.to_string()),
    )
    .await?;
    Ok((balance, row.credits))
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
    .await
    .map_err(map_sqlx)?;
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

const ORDER_COLUMNS: &str = "id, user_id, pack_code, credits, price_cents, amount_fen, status, out_trade_no, transaction_id, payer_openid, pay_channel";

fn order_view(
    row: &OrderRow,
    mock: bool,
    pay_params: Option<wechat_pay::JsapiPayParams>,
    virtual_payment: Option<VirtualPayParams>,
    balance: Option<i32>,
) -> OrderView {
    let price_label = find_pack(&row.pack_code)
        .map(|pack| pack.price_label.to_string())
        .unwrap_or_else(|_| format_fen(row.amount_fen));
    OrderView {
        order_id: row.id,
        out_trade_no: row.out_trade_no.clone(),
        pack_code: row.pack_code.clone(),
        credits: row.credits,
        price_cents: row.price_cents,
        amount_fen: row.amount_fen,
        price_label,
        status: row.status.clone(),
        mock,
        pay_params,
        virtual_payment,
        balance,
    }
}

fn format_fen(fen: i32) -> String {
    if fen % 100 == 0 {
        format!("¥{}", fen / 100)
    } else {
        format!("¥{:.2}", fen as f64 / 100.0)
    }
}

async fn user_openid(pool: &PgPool, user_id: Uuid) -> AppResult<String> {
    let openid: Option<String> = sqlx::query_scalar("SELECT openid FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    let openid = openid.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    if openid.trim().is_empty() {
        return Err(AppError::BadRequest("缺少微信 openid，无法下单".into()));
    }
    Ok(openid)
}

async fn order_by_trade_no(pool: &PgPool, out_trade_no: &str) -> AppResult<OrderRow> {
    let sql = format!("SELECT {ORDER_COLUMNS} FROM credit_orders WHERE out_trade_no = $1");
    sqlx::query_as::<_, OrderRow>(&sql)
        .bind(out_trade_no)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("订单不存在".into()))
}

async fn order_by_id(pool: &PgPool, order_id: Uuid) -> AppResult<OrderRow> {
    let sql = format!("SELECT {ORDER_COLUMNS} FROM credit_orders WHERE id = $1");
    sqlx::query_as::<_, OrderRow>(&sql)
        .bind(order_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("订单不存在".into()))
}

async fn lock_order_by_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order_id: Uuid,
) -> AppResult<OrderRow> {
    let sql = format!("SELECT {ORDER_COLUMNS} FROM credit_orders WHERE id = $1 FOR UPDATE");
    sqlx::query_as::<_, OrderRow>(&sql)
        .bind(order_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AppError::NotFound("订单不存在".into()))
}

async fn lock_order_by_trade_no(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    out_trade_no: &str,
) -> AppResult<OrderRow> {
    let sql =
        format!("SELECT {ORDER_COLUMNS} FROM credit_orders WHERE out_trade_no = $1 FOR UPDATE");
    sqlx::query_as::<_, OrderRow>(&sql)
        .bind(out_trade_no)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AppError::NotFound("订单不存在".into()))
}

fn map_sqlx(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db) = &err {
        if db.code().as_deref() == Some("23505") {
            return AppError::conflict("duplicate_payment", "重复的支付通知");
        }
    }
    AppError::from(err)
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
    price_cents: i32,
    amount_fen: i32,
    status: String,
    out_trade_no: String,
    transaction_id: Option<String>,
    payer_openid: Option<String>,
    pay_channel: String,
}

/// 锁内读取余额。供出稿事务复用。
pub async fn lock_balance(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
) -> AppResult<i32> {
    balance_of_tx(tx, user_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        let mut config = Config::from_env();
        config.force_mock_pay();
        config.wx_appid.clear();
        config.wx_secret.clear();
        config.wechat_app_id.clear();
        config
    }

    #[test]
    fn virtual_pay_wins_and_partial_virtual_blocks_mock() {
        let mut config = config();
        assert!(matches!(checkout_mode(&config), Checkout::Mock));

        config.wx_appid = "wx_test_app".into();
        config.wx_secret = "wx_test_secret".into();
        config.wx_virtual_offer_id = "offer-test".into();
        match checkout_mode(&config) {
            Checkout::Misconfigured(reason) => {
                assert!(reason.contains("WX_VIRTUAL_APPKEY_SANDBOX"));
            }
            _ => panic!("partial virtual config must not fall through"),
        }

        config.wx_virtual_appkey_sandbox = "sandbox-key".into();
        config.pay_credentials_present = true;
        config.wechat_mch_id = "1900000109".into();
        match checkout_mode(&config) {
            Checkout::Virtual(creds) => assert_eq!(creds.env, 1),
            _ => panic!("configured virtual pay is preferred over jsapi"),
        }
    }

    #[test]
    fn config_debug_does_not_print_appkey() {
        let mut config = config();
        let sentinel = "sentinel-appkey-not-real";
        config.wx_virtual_appkey_sandbox = sentinel.into();
        config.wx_virtual_msg_token = sentinel.into();
        let rendered = format!("{config:?}");
        assert!(!rendered.contains(sentinel));
    }
}
