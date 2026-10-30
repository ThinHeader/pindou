# 拼豆 API 草图

Base URL（本地）：`http://127.0.0.1:3000`

统一错误体：

```json
{ "error": "insufficient_credits", "message": "积分不足：需要 1，当前 0" }
```

## 健康检查

`GET /health` → `{ "status": "ok", "service": "pindou-api", "version": "..." }`

## 认证

### `POST /api/v1/auth/wx-login`

请求：`{ "code": "微信 wx.login 拿到的 code" }`

响应：

```json
{
  "token": "<JWT>",
  "user": { "id": "uuid", "nickname": "拼豆玩家", "avatar_url": null }
}
```

说明：未配置 `WX_APPID` / `WX_SECRET` 时走开发 stub，任意非空 `code` 即可。

## 积分

规则：

| 动作 | 变动 |
|------|------|
| 每日领取 | +3 |
| 成功出稿 | −1 |
| 激励广告 | +1 |
| 充值档位 | 占位，见下 |

### `GET /api/v1/credits/me`

```json
{
  "balance": 2,
  "daily_grant": 3,
  "generate_cost": 1,
  "ad_reward": 1,
  "daily_claimed_today": false
}
```

### `POST /api/v1/credits/daily-claim`

→ `{ "balance": 5, "delta": 3, "reason": "daily_claim" }`

### `POST /api/v1/credits/ad-reward`

→ `{ "balance": 3, "delta": 1, "reason": "ad_reward" }`

### 充值档位（占位，未实现支付）

| 档位 | 积分 | 示意价 |
|------|------|--------|
| 小包 | 10 | ¥6 |
| 中包 | 30 | ¥15 |
| 大包 | 68 | ¥30 |

## 出稿

### `POST /api/v1/drafts/generate`

请求：

```json
{
  "image_ref": "可选，对象键或临时 URL",
  "board_size": "52x52",
  "color_count": 24
}
```

成功：

```json
{
  "job_id": "uuid",
  "status": "queued",
  "balance": 1,
  "message": "出稿任务已入队（stub）"
}
```

积分不足：HTTP 402，`error: insufficient_credits`。

## 广场

### `GET /api/v1/plaza/feed`

→ `{ "items": [], "next_cursor": null }`（stub）

## 存储

COS / OSS 仅服务层接口占位（`services/storage.rs`），上传与 CDN URL 后续接入。
