-- 出稿进度增加 seedream（上传 → Seedream Flash → 拼豆像素）。
-- 保留 matting / cartoon，避免历史任务违反约束。新任务不再进入这两步。

DO $$
DECLARE
    constraint_name text;
BEGIN
    SELECT con.conname INTO constraint_name
    FROM pg_constraint con
    JOIN pg_class rel ON rel.oid = con.conrelid
    WHERE rel.relname = 'generation_jobs'
      AND con.contype = 'c'
      AND pg_get_constraintdef(con.oid) ILIKE '%status%';
    IF constraint_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE generation_jobs DROP CONSTRAINT %I', constraint_name);
    END IF;
END $$;

ALTER TABLE generation_jobs
    ADD CONSTRAINT generation_jobs_status_check
    CHECK (
        status IN (
            'queued',
            'upload',
            'matting',
            'cartoon',
            'seedream',
            'pixel',
            'done',
            'failed',
            'cancelled'
        )
    );
