-- 出稿失败的稳定业务错误码。版权或敏感内容拦截写 content_policy，其它失败留空。
ALTER TABLE generation_jobs
    ADD COLUMN error_code TEXT;
