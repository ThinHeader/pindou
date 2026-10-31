-- 拼豆 MVP 初始表结构

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    openid TEXT NOT NULL UNIQUE,
    nickname TEXT NOT NULL,
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE credit_accounts (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance INTEGER NOT NULL DEFAULT 0 CHECK (balance >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE credit_ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    delta INTEGER NOT NULL,
    balance_after INTEGER NOT NULL,
    reason TEXT NOT NULL,
    ref_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX credit_ledger_user_created ON credit_ledger (user_id, created_at DESC);

-- 日领取的数据库兜底（与 Redis SET NX 一起保证一天一次）
CREATE TABLE daily_claims (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    claim_date DATE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, claim_date)
);

CREATE TABLE drafts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    board_size INTEGER NOT NULL CHECK (board_size IN (52, 78, 104)),
    color_count INTEGER NOT NULL,
    peg_count INTEGER NOT NULL,
    palette JSONB NOT NULL,
    cells BYTEA NOT NULL,
    preview_key TEXT NOT NULL,
    source_key TEXT,
    palette_pref TEXT NOT NULL DEFAULT 'auto',
    is_template BOOLEAN NOT NULL DEFAULT FALSE,
    is_official BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX drafts_user_created ON drafts (user_id, created_at DESC);
CREATE INDEX drafts_official ON drafts (created_at DESC) WHERE is_official;
CREATE UNIQUE INDEX drafts_official_title ON drafts (title) WHERE is_official;

CREATE TABLE draft_favorites (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    draft_id UUID NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, draft_id)
);

CREATE TABLE works (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    draft_id UUID REFERENCES drafts(id) ON DELETE SET NULL,
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
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX works_created ON works (created_at DESC, id DESC);
CREATE INDEX works_category ON works (category, created_at DESC, id DESC);
CREATE INDEX works_likes ON works (like_count DESC, created_at DESC, id DESC);
CREATE UNIQUE INDEX works_draft_once ON works (draft_id) WHERE draft_id IS NOT NULL;

CREATE TABLE work_likes (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    work_id UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, work_id)
);

CREATE TABLE work_favorites (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    work_id UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, work_id)
);

CREATE TABLE work_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX work_comments_work ON work_comments (work_id, created_at DESC);

CREATE TABLE credit_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    pack_code TEXT NOT NULL,
    credits INTEGER NOT NULL,
    price_cents INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('created', 'paid')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    paid_at TIMESTAMPTZ
);

CREATE INDEX credit_orders_user ON credit_orders (user_id, created_at DESC);
