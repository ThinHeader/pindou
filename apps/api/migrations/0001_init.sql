-- 拼豆 API · SQLite 版全量初始结构
--
-- 由 Postgres 版 migrations_pg/0001..0009 合并而来。SQLite 没有独立类型系统，
-- 约定如下：
--   * UUID      → BLOB(16)，sqlx 的 Uuid 编解码就是 16 字节大端
--   * TIMESTAMPTZ → TEXT，UTC RFC3339（strftime 默认值，读出来是 DateTime<Utc>）
--   * DATE      → TEXT，YYYY-MM-DD
--   * JSONB     → TEXT，存 JSON 文本
--   * BYTEA     → BLOB
--   * BOOLEAN   → INTEGER 0/1
-- 进程连接时开启 foreign_keys 与 WAL；事务用 BEGIN IMMEDIATE 串行化写入，
-- 因此不再需要 Postgres 的 FOR UPDATE 与 RLS。

CREATE TABLE users (
    id BLOB PRIMARY KEY DEFAULT (randomblob(16)),
    openid TEXT NOT NULL UNIQUE,
    nickname TEXT NOT NULL,
    avatar_url TEXT,
    wx_session_key_enc TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE credit_accounts (
    user_id BLOB PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance INTEGER NOT NULL DEFAULT 0 CHECK (balance >= 0),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE credit_ledger (
    id BLOB PRIMARY KEY DEFAULT (randomblob(16)),
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    delta INTEGER NOT NULL,
    balance_after INTEGER NOT NULL,
    reason TEXT NOT NULL,
    ref_id TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX credit_ledger_user_created ON credit_ledger (user_id, created_at DESC);

-- 激励广告奖励幂等：每个用户每个东八区自然日一行。每日免费领取已停用。
CREATE TABLE daily_claims (
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    claim_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (user_id, claim_date)
);

CREATE TABLE drafts (
    id BLOB PRIMARY KEY DEFAULT (randomblob(16)),
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    board_size INTEGER NOT NULL CHECK (board_size IN (52, 78, 104)),
    color_count INTEGER NOT NULL,
    peg_count INTEGER NOT NULL,
    palette TEXT NOT NULL,
    cells BLOB NOT NULL,
    preview_key TEXT NOT NULL,
    source_key TEXT,
    palette_pref TEXT NOT NULL DEFAULT 'auto',
    is_template INTEGER NOT NULL DEFAULT 0,
    is_official INTEGER NOT NULL DEFAULT 0,
    pipeline TEXT NOT NULL DEFAULT 'pixel',
    provider TEXT NOT NULL DEFAULT 'local',
    remove_bg INTEGER NOT NULL DEFAULT 0,
    cartoon_strength TEXT NOT NULL DEFAULT 'off',
    subject_type TEXT NOT NULL DEFAULT 'object',
    palette_brand TEXT NOT NULL DEFAULT 'generic',
    matted_key TEXT,
    cartoon_key TEXT,
    pipeline_tags TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX drafts_user_created ON drafts (user_id, created_at DESC);
CREATE INDEX drafts_official ON drafts (created_at DESC) WHERE is_official = 1;
CREATE UNIQUE INDEX drafts_official_title ON drafts (title) WHERE is_official = 1;

CREATE TABLE draft_favorites (
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    draft_id BLOB NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (user_id, draft_id)
);

CREATE TABLE works (
    id BLOB PRIMARY KEY DEFAULT (randomblob(16)),
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    draft_id BLOB REFERENCES drafts(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    caption TEXT NOT NULL DEFAULT '',
    category TEXT NOT NULL DEFAULT 'pet',
    board_size INTEGER NOT NULL,
    color_count INTEGER NOT NULL,
    peg_count INTEGER NOT NULL,
    preview_key TEXT NOT NULL,
    like_count INTEGER NOT NULL DEFAULT 0,
    favorite_count INTEGER NOT NULL DEFAULT 0,
    comment_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX works_created ON works (created_at DESC, id DESC);
CREATE INDEX works_category ON works (category, created_at DESC, id DESC);
CREATE INDEX works_likes ON works (like_count DESC, created_at DESC, id DESC);
CREATE UNIQUE INDEX works_draft_once ON works (draft_id) WHERE draft_id IS NOT NULL;

CREATE TABLE work_likes (
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    work_id BLOB NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (user_id, work_id)
);

CREATE TABLE work_favorites (
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    work_id BLOB NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (user_id, work_id)
);

CREATE TABLE work_comments (
    id BLOB PRIMARY KEY DEFAULT (randomblob(16)),
    work_id BLOB NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX work_comments_work ON work_comments (work_id, created_at DESC);

CREATE TABLE credit_orders (
    id BLOB PRIMARY KEY DEFAULT (randomblob(16)),
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    pack_code TEXT NOT NULL,
    credits INTEGER NOT NULL,
    price_cents INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('created', 'paid', 'refunding', 'refunded')),
    out_trade_no TEXT NOT NULL,
    transaction_id TEXT,
    prepay_id TEXT,
    payer_openid TEXT,
    amount_fen INTEGER NOT NULL,
    pay_channel TEXT NOT NULL CHECK (pay_channel IN ('mock', 'wechat', 'virtual')),
    refund_order_id TEXT,
    refunded_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    paid_at TEXT
);

CREATE INDEX credit_orders_user ON credit_orders (user_id, created_at DESC);
CREATE UNIQUE INDEX credit_orders_out_trade_no_key ON credit_orders (out_trade_no);
CREATE UNIQUE INDEX credit_orders_transaction_id_key ON credit_orders (transaction_id)
    WHERE transaction_id IS NOT NULL;
CREATE UNIQUE INDEX credit_orders_refund_order_id_key ON credit_orders (refund_order_id)
    WHERE refund_order_id IS NOT NULL;

CREATE UNIQUE INDEX credit_ledger_recharge_once ON credit_ledger (reason, ref_id)
    WHERE reason = 'recharge' AND ref_id IS NOT NULL;
CREATE UNIQUE INDEX credit_ledger_refund_once ON credit_ledger (reason, ref_id)
    WHERE reason = 'refund' AND ref_id IS NOT NULL;

-- 出稿任务。小程序轮询 status：upload / seedream / pixel / done / failed / cancelled。
-- matting / cartoon 只为历史任务保留，新任务不再进入。
CREATE TABLE generation_jobs (
    id BLOB PRIMARY KEY,
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (
        status IN ('queued', 'upload', 'matting', 'cartoon', 'seedream', 'pixel', 'done', 'failed', 'cancelled')
    ),
    progress INTEGER NOT NULL DEFAULT 0,
    message TEXT NOT NULL DEFAULT '',
    error_message TEXT,
    failed_stage TEXT,
    error_code TEXT,
    board_size INTEGER NOT NULL,
    color_count INTEGER NOT NULL,
    palette_pref TEXT NOT NULL,
    remove_bg INTEGER NOT NULL,
    cartoon_strength TEXT NOT NULL,
    subject_type TEXT NOT NULL,
    palette_brand TEXT NOT NULL,
    title TEXT NOT NULL,
    provider TEXT NOT NULL,
    original_key TEXT,
    matted_key TEXT,
    cartoon_key TEXT,
    preview_key TEXT,
    draft_id BLOB REFERENCES drafts(id) ON DELETE SET NULL,
    balance_after INTEGER,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX generation_jobs_user_created ON generation_jobs (user_id, created_at DESC);
