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
| 403 | `forbidden` | 改、删、发布别人的稿，或改删官方模板 |
| 404 | `not_found` | 稿件 / 作品 / 订单不存在 |
| 409 | `already_claimed` | 今日积分已领 |

除健康检查、充值档位、广场列表 / 详情 / 评论列表、静态文件外，写操作都要带 `Authorization: Bearer <JWT>`。

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

返回用户、稿件 / 作品 / 收藏计数、积分快照、最近稿件和最近作品。

## 积分

| 动作 | 变动 |
|------|------|
| 每日领取 | +3，自然日（东八区）一次 |
| 成功出稿 | −1 |
| 激励广告 | +1，可多次 |
| 示意充值 | 见档位 |

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

无需登录。档位对齐设计稿：

| code | 积分 | 价格 |
|------|------|------|
| `small` | 10 | ¥6 |
| `popular` | 35 | ¥18（推荐） |
| `large` | 60 | ¥30 |

### `POST /api/v1/credits/orders`

`{ "pack_code": "small" }` → 订单 `status: "created"`，`mock: true`。

### `POST /api/v1/credits/orders/{id}/mock-callback`

开发态示意到账。重复调用不会重复加积分。正式支付上线前必须换成带签名校验的回调，这个接口不能暴露到生产。

## 上传与文件

### `POST /api/v1/uploads`

`multipart/form-data`，字段名 `file`。只接受能解码的 JPG / PNG / GIF。

```json
{ "key": "uploads/<uuid>.jpg", "url": "http://127.0.0.1:3000/files/uploads/<uuid>.jpg", "etag": "...", "content_type": "image/jpeg" }
```

这是 COS PutObject 的字段形状。本地实现写到 `STORAGE_DIR`。

### `GET /files/{key}`

读取上面的对象。预览图键形如 `previews/<uuid>.png`。

## 出稿

### `POST /api/v1/drafts/generate`

```json
{
  "image_ref": "uploads/....jpg 或 builtin:cat",
  "image_base64": "可选，data URL 或纯 base64",
  "board_size": 52,
  "color_count": 12,
  "palette_pref": "auto",
  "title": "可选"
}
```

- `board_size` 也可以写 `"52x52"` / `"78×78"` / `"104x104"`。只接受 52、78、104。
- `color_count` 默认 12，范围 4–24，表示色数上限；实际色数可能更少。
- `palette_pref`：`auto` | `warm` | `macaron` | `contrast`。
- `image_ref`：`builtin:cat|bear|chick|bunny` 用内置示例图。
- `title`：可选，1–40 字，首尾空格会去掉。不传或只传空白时：示例图用「小橘猫吐舌 / 黄油小熊 / 柠檬汽水 / 蜜桃兔耳」，其他图用「未命名草稿」。超过 40 字返回 400，且不扣积分。
- 没图：400「请先选择图片」。坏图：400「无法解析图片，请上传 JPG 或 PNG」。过大：400「图片过大…」。
- 先量化再扣分。图片失败不扣积分。余额不足 402。标题不合法发生在扣分之前。

成功：

```json
{
  "job_id": "uuid",
  "draft_id": "uuid",
  "status": "ready",
  "balance": 2,
  "message": "色号稿已生成（52×52，实际 9 色）",
  "draft": { "id": "...", "palette": [], "preview_url": "...", "peg_count": 2704 }
}
```

算法：缩放到珠板尺寸后，映射到内置拼豆色板（含设计稿色号 A1 / B3 / C2 / D1 / K9 等），再把超出色数上限的小色块并进最近色。预览是一颗颗珠子的 PNG。

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

只改稿件本身。已经发布到广场的作品标题不会跟着变，发布当时写到作品上的标题保持原样。

### `DELETE /api/v1/drafts/{id}`

删除自己的稿，无需请求体。

```json
{ "deleted": true, "work_deleted": false }
```

- 未发布：删稿件，稿件收藏随外键级联。`work_deleted` 为 `false`。
- 已发布：同一事务里先删广场作品（点赞、收藏、评论随作品外键级联），再删稿件。`work_deleted` 为 `true`。选择连作品一起删，是因为目前没有单独的下架入口，只拒绝会让已发布稿永远删不掉。
- 预览图对象在事务提交后删除。上传的原图保留。
- 不是自己的稿：403「只能删除自己的稿件」。官方模板：403「官方模板不能删除」。不存在：404。

### `POST /api/v1/drafts/{id}/favorite`

切换收藏。`{ "favorited": true }`

### `POST /api/v1/drafts/{id}/publish`

```json
{ "title": "可选，1–40 字", "caption": "可选", "category": "pet" }
```

`category`：`pet` 萌宠 / `food` 食物 / `pixel` 像素人 / `festival` 节日 / `simple` 简约。只能发布自己的稿。同一稿重复发布返回原作品，不会改标题。

`title` 不传或空白时，作品沿用稿件当前标题。传了非空标题时，作品用这个标题，并在同一事务里写回稿件，避免稿件库仍显示「未命名草稿」。超过 40 字返回 400，不会发布。

## 广场

### `GET /api/v1/plaza/feed?tab=recommend|hot|pet|food|pixel|festival|simple&q=&cursor=&limit=`

无需登录。`hot` 按点赞排序，其余按时间。`next_cursor` 可翻页。登录后 `liked` / `favorited` 会标出当前用户。

### `GET /api/v1/plaza/works/{id}`

### `POST /api/v1/plaza/works/{id}/like`

`{ "liked": true, "like_count": 1 }`，再调一次取消。

### `POST /api/v1/plaza/works/{id}/favorite`

`{ "favorited": true, "favorite_count": 1 }`

### `GET /api/v1/plaza/works/{id}/comments`

### `POST /api/v1/plaza/works/{id}/comments`

`{ "content": "1 到 200 字" }`

启动时如果库里还没有官方模板，会写入「小橘猫吐舌 / 黄油小熊 / 柠檬汽水 / 蜜桃兔耳」和对应广场作品，方便直接浏览。
