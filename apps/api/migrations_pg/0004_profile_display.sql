-- 「我的」页展示用的昵称。
-- users.nickname 仍是既有作者名（广场 / 稿件），这里不改它，
-- 避免把头像昵称填写能力收集到的昵称公开出去。
-- avatar_url 继续可空：设置头像时写入对象存储 key（uploads/...），不写外部 URL。
-- 登录和 /me 再把 key 解析成文件地址；广场作者不返回头像。

ALTER TABLE users ADD COLUMN profile_nickname TEXT;
