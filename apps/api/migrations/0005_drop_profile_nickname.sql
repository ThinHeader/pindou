-- 头像昵称在登录时写入 users.nickname / users.avatar_url。
-- profile_nickname 是上一版单独的展示名，广场已关闭，不再需要第二列。

UPDATE users
SET nickname = TRIM(profile_nickname)
WHERE profile_nickname IS NOT NULL
  AND TRIM(profile_nickname) <> '';

ALTER TABLE users DROP COLUMN profile_nickname;
