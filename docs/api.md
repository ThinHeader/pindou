# 拼豆 API

Base URL（本地）：`http://127.0.0.1:3000`

统一错误：

```json
{ "error": "insufficient_credits", "message": "积分不足：需要 1，当前 0" }
```

| HTTP | error | 场景 |
|------|--------|------|
| 400 | `bad_request` | 参数、图片无法解析、未选图、稿件名称为空或超过 40 字 |
| 401 | `unauthorized` | 未登录或 token 失效 |
| 402 | `insufficient_credits` | 出稿时余额不足 |
| 403 | `forbidden` | 改、删别人的稿，或改删官方模板 |
| 404 | `not_found` | 稿件 / 订单不存在 |
| 409 | `already_claimed` | 今日积分已领 |
| 410 | `gone` | 作品发布、作品流、点赞、收藏、评论已关闭 |

除健康检查、充值档位、静态文件外，写操作都要带 `Authorization: Bearer <JWT>`。作品发布和作品互动接口不校验登录，直接返回 410。

## 健康检查

`GET /health`

```json
{ "status": "ok", "service": "pindou-api", "version": "0.1.0" }
```

## 登录

### `POST /api/v1/auth/wx-login`

```json
{ "code": "wx.login 的 code，或开发态稳定 code" }
```

```json
{
  "token": "<JWT，7 天>",
  "mode": "dev",
  "user": { "id": "uuid", "nickname": "拼豆玩家", "avatar_url": null }
}
```

- `WX_APPID` 与 `WX_SECRET` 都非空：请求微信 `jscode2session`，用返回的 openid。
- 否则为开发态：`mock:` 前缀会原样进 openid；其他 code 做稳定哈希。同一 code 永远是同一用户。空 code 返回 400。

### `GET /api/v1/me` · `PUT /api/v1/me`

`PUT` 体：`{ "nickname": "豆豆手作" }`（1–20 字）。同样接受 `PATCH`，小程序用 `PUT`（`uni.request` 没有 PATCH）。

返回用户、稿件 / 历史作品行 / 收藏计数、积分快照和最近稿件。不再返回作品列表。

## 积分

| 动作 | 变动 |
|------|------|
| 每日领取 | +3，自然日（东八区）一次 |
| 成功出稿 | −1 |
| 激励广告 | +1，可多次 |
| 充值 | 见档位。开发示意或微信支付，都只入账一次 |

日领在同一事务里向 `daily_claims` 插入 `(user_id, 东八区今日日期)`；主键冲突则返回 409 `already_claimed`，成功则给账户 +3 并写入 `credit_ledger`。不依赖 Redis。

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

`{ "balance": 5, "delta": 3, "reason": "daily_claim" }`

### `POST /api/v1/credits/ad-reward`

`{ "balance": 3, "delta": 1, "reason": "ad_reward" }`

### `GET /api/v1/credits/ledger`

`{ "items": [{ "delta": 3, "reason": "daily_claim", "reason_label": "每日领取", "balance_after": 3, "created_at": "..." }] }`

### `GET /api/v1/credits/packs`

无需登录。档位：

| code | 积分 | 价格 |
|------|------|------|
| `small` | 10 | ¥6（600 分） |
| `popular` | 35 | ¥18（推荐） |
| `large` | 60 | ¥30 |

响应里的 `pay_mode`：

| 值 | 含义 |
|----|------|
| `mock` | 未配置任何微信支付商户参数。可以走示意回调 |
| `wechat` | 已启用 JSAPI。下单返回调起支付参数 |
| `misconfigured` | 写了部分商户参数。示意到账关闭，下单 503 |

### `POST /api/v1/credits/orders`

`{ "pack_code": "small" }`。只创建 `status: "created"` 的订单，**不加积分**。

开发示意时 `mock: true`，没有 `pay_params`。

微信支付时 `mock: false`，并带上小程序 `wx.requestPayment` 参数：

```json
{
  "order_id": "uuid",
  "out_trade_no": "32 位商户单号",
  "amount_fen": 600,
  "status": "created",
  "mock": false,
  "pay_params": {
    "timeStamp": "1710000000",
    "nonceStr": "...",
    "package": "prepay_id=wx...",
    "signType": "RSA",
    "paySign": "..."
  }
}
```

`paySign` 是商户私钥对 `appId\n时间戳\n随机串\nprepay_id=...\n` 的 SHA256withRSA。付款人 `openid` 来自登录用户，不由客户端上传。

### `GET /api/v1/credits/orders/{id}`

订单所属用户查询状态。`paid` 之前不加积分。正式支付且 API 指向 `api.mch.weixin.qq.com` 时，若本地仍是 `created`，会向微信查单；查单响应验签通过且交易成功才补入账。响应带当前 `balance`，方便支付后轮询。

### `POST /api/v1/credits/orders/{id}/mock-callback`

仅 `pay_mode=mock` 且订单渠道是 `mock` 时入账。重复调用 `delta` 为 0。配置了任一商户参数后返回 403，不加积分。

### `POST /api/v1/pay/wechat/notify`

微信支付结果通知，无登录态。用 `Wechatpay-Timestamp` / `Nonce` / `Signature` / `Serial` 验签（5 分钟时间窗），再用 APIv3 密钥做 AES-256-GCM 解密。

- 签名无效：401，不加积分。
- 金额、AppID、商户号或 `openid` 与订单不一致：4xx/5xx，不加积分，订单保持 `created`。
- `TRANSACTION.SUCCESS` 且核对通过：同一事务把订单标为 `paid` 并写 `recharge` 流水。
- 同一 `transaction_id` 或同一订单的重复通知：200，不再加积分。

成功响应 `{"code":"SUCCESS","message":"成功"}`。

## 上传与文件

### `POST /api/v1/uploads`

`multipart/form-data`，字段名 `file`。只接受能解码的 JPG / PNG / GIF。

```json
{ "key": "uploads/<uuid>.jpg", "url": "http://127.0.0.1:3000/files/uploads/<uuid>.jpg", "etag": "...", "content_type": "image/jpeg" }
```

这是 COS PutObject 的字段形状。本地实现写到 `STORAGE_DIR`。

### `GET /files/{key}`

读取上面的对象。预览图键形如 `previews/<uuid>.png`。

## 出稿（MVP+）

一次成功出稿只扣 1 积分，包含智能去背、人物动漫化和拼豆量化。失败或取消不扣，也不写退款（因为没有预扣）。

`POST /api/v1/drafts/generate` 校验参数和图片后立刻返回任务，管线在后台跑。小程序轮询 `GET /api/v1/jobs/{id}`。

### `POST /api/v1/drafts/generate`

```json
{
  "image_ref": "uploads/....jpg 或 builtin:cat",
  "image_base64": "可选，data URL 或纯 base64",
  "board_size": 52,
  "color_count": 12,
  "palette_pref": "auto",
  "title": "可选",
  "remove_bg": true,
  "cartoon_strength": "standard",
  "subject_type": "person",
  "palette_brand": "generic"
}
```

- `board_size` 也可以写 `"52x52"` / `"78×78"` / `"104x104"`。只接受 52、78、104。
- `color_count` 默认 12，范围 4–24，表示色数上限；稿件上的 `color_count` 是实际色数，可能更少。
- `palette_pref`：`auto` | `warm` | `macaron` | `contrast`。
- `remove_bg` 默认 `true`。关掉后跳过去背，卡通直接吃原图。
- `cartoon_strength`：`light` 轻 / `standard` 标准（默认）/ `strong` 强。
- `subject_type`：`person` 人像（人体分割）/ `pet` 宠物 / `object` 静物（后两者走通用分割）。默认人像。
- `palette_brand`：`generic` 通用拼豆 / `mard` / `perler` / `shop` 店内色卡。都是内置 SKU 的子集，不是厂商目录抓取。
- `image_ref`：`builtin:cat|bear|chick|bunny` 用内置示例图。
- `title`：可选，1–40 字，首尾空格会去掉。不传或只传空白时：示例图用「小橘猫吐舌 / 黄油小熊 / 柠檬汽水 / 蜜桃兔耳」，其他图用「未命名草稿」。超过 40 字返回 400，且不建任务、不扣积分。
- 没图：400「请先选择图片」。坏图：400「无法解析图片，请上传 JPG 或 PNG」。过大：400「图片过大…」。余额不足 402。这些都发生在扣分之前。
- 没配阿里云密钥时 `provider` 为 `mock`：本地抠出透明 PNG、做色阶卡通，再用最近邻得到真实色号稿。
- `PIPELINE_PROVIDER=aliyun` 但密钥为空：400，不建任务。

立刻返回的任务（此时通常还没扣分）：

```json
{
  "job_id": "uuid",
  "status": "matting",
  "progress": 36,
  "message": "智能去背中",
  "provider": "mock",
  "charged": false,
  "stages": [
    { "id": "upload", "label": "上传原图", "state": "done", "detail": "原图已就绪" },
    { "id": "matting", "label": "智能去背", "state": "active", "detail": "开发模拟去背中" },
    { "id": "cartoon", "label": "人物动漫化", "state": "pending", "detail": "人物动漫化" },
    { "id": "pixel", "label": "拼豆量化", "state": "pending", "detail": "自研像素 / 色板映射" }
  ]
}
```

`stages[].state`：`done` | `active` | `pending` | `fail` | `skipped`（去背关闭）。

### `GET /api/v1/jobs/{id}`

只能看自己的任务。别人的任务 404。

`status`：`matting` | `cartoon` | `pixel` | `done` | `failed` | `cancelled`（提交后也可能短暂停在 `upload` / `queued`）。

完成后 `charged` 为 true，`balance` 是扣分后的余额，`draft` 是稿件详情。`original_url` / `matted_url` / `cartoon_url` / `preview_url` 指向对象存储里的中间图和珠图。去背关闭时 `matted_url` 为 null。

失败时 HTTP 仍是 200，`status` 为 `failed`，`error` 说明原因，并写明本次未扣积分。`failed_stage` 会反映在对应步骤的 `state: "fail"` 上。

### `POST /api/v1/jobs/{id}/cancel`

进行中的任务改为 `cancelled`，文案「已取消，未扣积分」。已经 `done` 的任务保持完成态，不会把积分退回去。

珠图算法：把卡通图（或未去背时的原图）最近邻缩放到珠板网格，映射到内置拼豆色板（含 A1 / B3 / C2 / D1 / K9 等），再把超出色数上限的小色块并进最近色。不用扩散模型产生官方珠图。预览是一颗颗珠子的 PNG。

稿件详情额外字段：`pipeline`（`mvp_plus`；旧的官方模板是 `pixel`）、`provider`（`mock` | `aliyun` | `local`）、`remove_bg`、`cartoon_strength`、`subject_type`、`palette_brand`、`pipeline_tags`、`pipeline_lines`、`original_url`、`matted_url`、`cartoon_url`、`source_key`。列表卡片带 `pipeline_tags`，官方模板为空数组。

## 稿件库

### `GET /api/v1/drafts?tab=all|mine|favorite|template|official&q=`

`{ "items": [DraftCard], "next_cursor": null }`，最多 60 条。

### `GET /api/v1/drafts/{id}`

含 `palette`、`cells_b64`（行优先调色板下标）、`hint`、`work_id`、`favorited`。`badge` 为「我的」表示当前用户自己的稿（官方模板和收藏别人的稿不会是这个徽章）。

### `PUT /api/v1/drafts/{id}` · `PATCH /api/v1/drafts/{id}`

改自己的稿件名称。小程序用 `PUT`（`uni.request` 没有 PATCH）；两种方法同一处理。

```json
{ "title": "窗边的猫" }
```

返回更新后的稿件详情。名称 1–40 字。空标题 400「请填写稿件名称」。超长 400「稿件名称请控制在 40 字以内」。不是自己的稿 403「只能修改自己的稿件」。官方模板 403「官方模板不能修改」。不存在 404。

只改稿件本身。作品发布已关闭，改名不会再写入作品。

### `DELETE /api/v1/drafts/{id}`

删除自己的稿，无需请求体。

```json
{ "deleted": true, "work_deleted": false }
```

- 没有关联作品行：删稿件，稿件收藏随外键级联。`work_deleted` 为 `false`。
- 历史上关联过作品行：同一事务里先删这些行（点赞、收藏、评论随作品外键级联），再删稿件。`work_deleted` 为 `true`。新的发布入口已关闭，正常出稿不会再产生这种关联。
- 预览图对象在事务提交后删除。上传的原图保留。
- 不是自己的稿：403「只能删除自己的稿件」。官方模板：403「官方模板不能删除」。不存在：404。

### `POST /api/v1/drafts/{id}/favorite`

切换收藏。`{ "favorited": true }`

### `POST /api/v1/drafts/{id}/publish`

已关闭。任意调用返回 `410`，`error` 为 `gone`，文案「该功能已关闭」。不会创建作品，也不会改稿件标题。

## 已关闭的作品互动

以下接口一律 `410 Gone`，`error` 为 `gone`。个人主体不提供用户内容发布、点赞、收藏和评论。

- `GET /api/v1/plaza/feed`
- `GET /api/v1/plaza/works/{id}`
- `POST /api/v1/plaza/works/{id}/like`
- `POST /api/v1/plaza/works/{id}/favorite`
- `GET /api/v1/plaza/works/{id}/comments`
- `POST /api/v1/plaza/works/{id}/comments`

启动时如果库里还没有官方模板，会写入「小橘猫吐舌 / 黄油小熊 / 柠檬汽水 / 蜜桃兔耳」，只出现在稿件库，不再写入作品或评论。
