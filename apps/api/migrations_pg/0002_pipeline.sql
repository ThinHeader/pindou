-- MVP+ 出稿管线：任务进度 + 稿件上的去背 / 卡通 / 色盘标记。
-- 积分仍只在成功落稿的事务里扣 1 分；失败和取消不写扣减，因此也不退款。

ALTER TABLE drafts
    ADD COLUMN pipeline TEXT NOT NULL DEFAULT 'pixel',
    ADD COLUMN provider TEXT NOT NULL DEFAULT 'local',
    ADD COLUMN remove_bg BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN cartoon_strength TEXT NOT NULL DEFAULT 'off',
    ADD COLUMN subject_type TEXT NOT NULL DEFAULT 'object',
    ADD COLUMN palette_brand TEXT NOT NULL DEFAULT 'generic',
    ADD COLUMN matted_key TEXT,
    ADD COLUMN cartoon_key TEXT,
    ADD COLUMN pipeline_tags JSONB NOT NULL DEFAULT '[]'::jsonb;

-- 出稿任务。小程序轮询 status：upload / matting / cartoon / pixel / done / failed / cancelled。
CREATE TABLE generation_jobs (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (
        status IN ('queued', 'upload', 'matting', 'cartoon', 'pixel', 'done', 'failed', 'cancelled')
    ),
    progress INTEGER NOT NULL DEFAULT 0,
    message TEXT NOT NULL DEFAULT '',
    error_message TEXT,
    failed_stage TEXT,
    board_size INTEGER NOT NULL,
    color_count INTEGER NOT NULL,
    palette_pref TEXT NOT NULL,
    remove_bg BOOLEAN NOT NULL,
    cartoon_strength TEXT NOT NULL,
    subject_type TEXT NOT NULL,
    palette_brand TEXT NOT NULL,
    title TEXT NOT NULL,
    provider TEXT NOT NULL,
    original_key TEXT,
    matted_key TEXT,
    cartoon_key TEXT,
    preview_key TEXT,
    draft_id UUID REFERENCES drafts(id) ON DELETE SET NULL,
    balance_after INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX generation_jobs_user_created ON generation_jobs (user_id, created_at DESC);

-- 表所有者（API 使用的数据库角色）绕过 RLS。不给 anon 开策略，避免 Data API 直接读写任务。
ALTER TABLE generation_jobs ENABLE ROW LEVEL SECURITY;
