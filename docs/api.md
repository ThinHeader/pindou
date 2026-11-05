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
| 409 | `already_claimed` | 今日激励广告奖励已领 |
| 409 | `refund_failed` | 客诉退款被微信拒绝，可再试 |
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

### `GET /api/v1/me` · `PUT /api/v1/me` · `POST /api/v1/me/profile`

点「微信登录」时先调 `wx.getUserProfile`（用途说明不超过 30 字）。基础库 2.27.1 起该接口不再弹授权窗，昵称固定为「微信用户」、头像为默认灰图，客户端丢弃这份匿名结果，并立刻打开微信规定的头像昵称填写（`chooseAvatar` + `type="nickname"`）。换票后用 `PUT /api/v1/me` 写入头像和昵称。两者都只出现在登录响应和 `/me`，用来在「我的」展示当前账号。

`PUT`（也接受 `PATCH`）的请求体是 JSON，和小程序 `wx.login` 换票同一条 `wx.request`：

```json
{ "nickname": "肚圆", "avatar_base64": "<标准 base64，可带 data URL 前缀>" }
```

- `nickname`：1–20 个字。只改昵称时必填。
- `avatar_base64`：可选，JPG / PNG / GIF。解码后与出稿参考图一样写入对象存储，库里只存对象键。和头像一起提交时，空昵称表示不改昵称。

小程序不能用 `wx.uploadFile`，也不能用 `wx.request` 发 `ArrayBuffer` 的 multipart：这两种在真机上都会在收到 HTTP 响应之前失败（toast「保存失败，请重试」），登录 JSON 却是成功的。所以登录页把 `chooseAvatar` 临时文件读成 base64，放进上面的 JSON。

`POST /api/v1/me/profile` 仍是 `multipart/form-data`，给已经能上传文件的客户端：

- `nickname`：可选，1–20 个字。空则不改昵称。
- `file`：可选，JPG / PNG / GIF。服务写入现有对象存储（与出稿参考图相同的 `key` / `url`），库里只存对象键。

至少带昵称或头像之一。响应里的 `user.avatar_url` 是可访问的文件 URL。换 `PUBLIC_BASE_URL` 后下次读取会按新地址拼。

返回用户、稿件 / 历史作品行 / 收藏计数、积分快照和最近稿件。不再返回作品列表。

## 积分

| 动作 | 变动 |
|------|------|
| 成功出稿 | −1 |
| 激励广告 | +1，东八区每个自然日一次 |
| 充值 | 见档位。开发示意、虚拟支付或微信支付，都只入账一次 |
| 客诉退款 | 虚拟支付整单退款成功后扣回该档积分（见下文，不对用户开放） |

每日免费领取已停用，`POST /api/v1/credits/daily-claim` 不存在。激励广告在同一事务里向 `daily_claims` 插入 `(user_id, 东八区今日日期)`；主键冲突则返回 409 `already_claimed`，成功则给账户 +1 并写入 `credit_ledger`（`reason=ad_reward`）。不依赖 Redis。历史流水里的 `daily_claim` 仍显示为「每日领取」。

小程序必须先把激励视频播完（`uni.createRewardedVideoAd` 的 `onClose` 且 `isEnded`）再调用发积分接口。服务端不信客户端之外的凭证，只保证同一用户同一天只入账一次。广告位 ID 放在小程序 `VITE_REWARD_AD_UNIT_ID`，留空时客户端不会请求发积分。

### `GET /api/v1/credits/me`

```json
{
  "balance": 2,
  "generate_cost": 1,
  "ad_reward": 1,
  "ad_claimed_today": false
}
```

### `POST /api/v1/credits/ad-reward`

需要登录。`{ "balance": 1, "delta": 1, "reason": "ad_reward" }`

当天再领返回 409 `already_claimed`。

### `GET /api/v1/credits/ledger`

`{ "items": [{ "delta": 1, "reason": "ad_reward", "reason_label": "激励广告", "balance_after": 1, "created_at": "..." }] }`

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
| `virtual` | 已启用虚拟支付。下单返回 `virtual_payment`，优先于 JSAPI |
| `mock` | 未配置虚拟支付，也未配置任何微信支付商户参数。可以走示意回调 |
| `wechat` | 未启用虚拟支付，已启用 JSAPI。下单返回调起支付参数 |
| `misconfigured` | 虚拟支付或商户参数只写了一部分。示意到账关闭，下单 503 |

### `POST /api/v1/credits/orders`

`{ "pack_code": "small" }`。只创建 `status: "created"` 的订单，**不加积分**。

开发示意时 `mock: true`，没有 `pay_params`，也没有 `virtual_payment`。

虚拟支付时 `mock: false`，并带上小程序 `wx.requestVirtualPayment` 参数。`signData` 是参与签名的原文字符串，客户端不能再 `JSON.stringify` 一次：

```json
{
  "order_id": "uuid",
  "out_trade_no": "32 位商户单号",
  "amount_fen": 600,
  "status": "created",
  "mock": false,
  "virtual_payment": {
    "signData": "{\"offerId\":\"...\",\"buyQuantity\":1,\"env\":1,\"currencyType\":\"CNY\",\"productId\":\"small\",\"goodsPrice\":600,\"outTradeNo\":\"...\",\"attach\":\"pack=small&order=...\"}",
    "paySig": "hex hmac-sha256",
    "signature": "hex hmac-sha256",
    "mode": "short_series_goods"
  }
}
```

`paySig` 是 AppKey 对 `requestVirtualPayment&` + `signData` 的 HMAC-SHA256，十六进制。`signature` 是该用户 `session_key` 对同一段 `signData` 的 HMAC-SHA256。响应里没有 AppKey，也没有 `session_key`。没有 `session_key` 时下单返回 401，需要重新微信登录。

道具 ID 默认等于档位 `small` / `popular` / `large`，`goodsPrice` 分别是 600 / 1800 / 3000。MP 道具价格必须一致。

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

订单所属用户查询状态。`paid` 之前不加积分。JSAPI 且 API 指向 `api.mch.weixin.qq.com` 时，若本地仍是 `created`，会向微信查单；查单响应验签通过且交易成功才补入账。虚拟支付订单同样会查 `/xpay/query_order`，已支付才补入账，并通知微信已发货。响应带当前 `balance`，方便支付后轮询。

### `POST /api/v1/credits/orders/{id}/mock-callback`

仅 `pay_mode=mock` 且订单渠道是 `mock` 时入账。重复调用 `delta` 为 0。配置了任一商户参数或虚拟支付参数后返回 403，不加积分。

### `GET|POST /api/v1/pay/virtual/notify`

虚拟支付道具发货推送，无登录态。地址是 `{PUBLIC_BASE_URL}/api/v1/pay/virtual/notify`。

- GET：用 `WX_VIRTUAL_MSG_TOKEN` 校验消息推送签名后回显 `echostr`。
- POST：事件 `xpay_goods_deliver_notify`。配了 Token 时先验签。订单、openid、道具、金额、环境一致后，优先查单确认再入账；查单不可用但推送已验签时，按推送入账。重复通知返回 `{"ErrCode":0,"ErrMsg":"success"}`，不加第二次积分。
- 签名无效或尚未支付：`ErrCode` 非 0，不加积分。
- `xpay_refund_notify` 且 `RetCode=0`：核对订单后把订单标为 `refunded`，并扣回已发积分（只扣一次）。失败通知不扣积分。字段不全的退款推送只确认收到。

### `POST /api/v1/ops/credit-orders/{key}/refund`

客诉 / 运维退款，**不是**小程序接口。不要放进小程序，也不要接受用户的登录 JWT。

- 请求头 `X-Ops-Token` 必须等于环境变量 `OPS_REFUND_TOKEN`。未配置返回 503。口令错误返回 401，即使带了用户 `Authorization` 也不行。
- `{key}` 可以是订单 UUID，也可以是 32 位 `out_trade_no`（两者是同一笔订单的两种写法）。
- 只接受 `pay_channel=virtual` 且已支付的订单。开发示意单和商户 JSAPI 单返回 400。
- 向微信 `POST /xpay/refund_order` 发起**整单**退款（`refund_fee=left_fee`）。`env`、OfferID、AppKey 与查单相同：沙箱用 `WX_VIRTUAL_APPKEY_SANDBOX`，`env=1`。签名是 AppKey 对 `/xpay/refund_order&` + 请求体的 HMAC-SHA256。`refund_reason=2`（售后），`req_from=1`（人工客服）。
- 微信这个接口只表示退款任务已启动。服务会再查退款单；状态 5 或 8 才把本地订单改为 `refunded`，并写一条 `refund` 流水扣回积分。账户不够扣时扣到 0，响应里会说明未扣满。
- 任务还在处理时返回 `status: "refunding"`、`delta: 0`。用同一条命令再调一次即可，不会重复退款，也不会重复扣积分。已经退过的订单再次调用 `delta` 为 0。

```bash
curl -X POST "http://127.0.0.1:3000/api/v1/ops/credit-orders/订单UUID或商户单号/refund" \
  -H "X-Ops-Token: $OPS_REFUND_TOKEN"
```

```json
{
  "order_id": "uuid",
  "out_trade_no": "32 位商户单号",
  "status": "refunded",
  "refund_order_id": "r…",
  "balance": 0,
  "delta": -10,
  "message": "已退款，扣回 10 积分"
}
```

微信侧前置条件：小程序已开通虚拟支付；沙箱退款要用开发版或体验版的沙箱 AppKey（`WX_VIRTUAL_ENV` 为空或 `1`）。资金管理里待结算的沙箱金额可以退回。正式环境退款需在虚拟支付后台具备退款能力，支付 365 天内可退，超过 180 天平台不退手续费。口令和 AppKey 只放环境变量，不要写进仓库。

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

读取上面的对象。`key` 带目录前缀：上传图是 `uploads/<uuid>.<ext>`，对应地址是 `/files/uploads/<uuid>.<ext>`。只写 `/files/<uuid>.<ext>` 会 404，文件实际在存储目录的 `uploads/` 下。预览图键形如 `previews/<uuid>.png`，Seedream 中间图键形如 `pipeline/<job>/seedream.png`。

## 出稿（MVP+）

一次成功出稿只扣 1 积分，包含火山方舟 Seedream Flash 图生图和拼豆量化。失败或取消不扣，也不写退款（因为没有预扣）。

`POST /api/v1/drafts/generate` 校验参数和图片后立刻返回任务，管线在后台跑。小程序轮询 `GET /api/v1/jobs/{id}`。

路径是：上传原图 → `POST https://ark.cn-beijing.volces.com/api/v3/images/generations`（模型 `doubao-seedream-5-0-flash-260915`）→ 自研最近邻。方舟返回约 1K 平涂图。珠板格数只看 `board_size`（52 / 78 / 104），不看提示词，也不看 API 图的像素尺寸。Flash 大约 ¥0.12 / 张。不要用阿里云视觉智能或 VIAPI 的密钥。

图生图的 `image` 是请求体内的 data URL（`data:image/...;base64,...`），用的是服务已经读到的原图字节。不会把 `PUBLIC_BASE_URL/files/...` 交给方舟拉取：方舟在国内，公网域名在海外或指到本机时会超时（`Timeout while down`）。

### `POST /api/v1/drafts/generate`

```json
{
  "image_ref": "uploads/....jpg 或 builtin:cat",
  "image_base64": "可选，data URL 或纯 base64",
  "board_size": 52,
  "color_count": 12,
  "palette_pref": "auto",
  "title": "可选",
  "subject_type": "person",
  "palette_brand": "generic"
}
```

- `board_size` 也可以写 `"52x52"` / `"78×78"` / `"104x104"`。只接受 52、78、104。这就是最终珠图边长。
- `color_count` 默认 12，范围 4–24，表示色数上限；稿件上的 `color_count` 是实际色数，可能更少。
- `palette_pref`：`auto` | `warm` | `macaron` | `contrast`。
- `subject_type`：`person` 人像 / `pet` 宠物 / `object` 静物。默认人像。只写入标签，不改变 Seedream 提示词。
- `palette_brand`：`generic` 通用拼豆 / `mard` / `perler` / `shop` 店内色卡。都是内置 SKU 的子集，不是厂商目录抓取。
- `remove_bg`、`cartoon_strength` 若仍被旧客户端带上，会被忽略，不再 400，也不再分出去背或卡通步骤。
- `image_ref`：`builtin:cat|bear|chick|bunny` 用内置示例图。
- `title`：可选，1–40 字，首尾空格会去掉。不传或只传空白时：示例图默认名是「小橘猫吐舌」（`builtin:cat`）、「黄油小熊」（`builtin:bear`）、「柠檬汽水」（`builtin:chick`）、「蜜桃兔耳」（`builtin:bunny`），其他图用「未命名草稿」。这些名字只出现在用户自己的出稿上，启动时不会落成官方稿。超过 40 字返回 400，且不建任务、不扣积分。
- 没图：400「请先选择图片」。坏图：400「无法解析图片，请上传 JPG 或 PNG」。过大：400「图片过大…」。余额不足 402。这些都发生在扣分之前。
- 没配 `ARK_API_KEY` 且 `PIPELINE_PROVIDER=auto`（或显式 `mock`）时 `provider` 为 `mock`：本地做出透明平涂 PNG，再用最近邻得到真实色号稿，不请求方舟。
- `PIPELINE_PROVIDER=seedream` 或 `ark` 但 `ARK_API_KEY` 为空：400「出稿服务未就绪。本次未扣积分。」，不建任务，不扣积分。上游报错也不会把原文回给小程序。
- `PIPELINE_PROVIDER=aliyun`：400，文案是「当前出稿方式已停用。本次未扣积分。」这条路径已退出。

立刻返回的任务（此时通常还没扣分）：

```json
{
  "job_id": "uuid",
  "status": "seedream",
  "progress": 36,
  "message": "正在整理色块",
  "provider": "mock",
  "charged": false,
  "stages": [
    { "id": "upload", "label": "上传原图", "state": "done", "detail": "原图已就绪" },
    { "id": "seedream", "label": "色块整理", "state": "active", "detail": "本地整理色块" },
    { "id": "pixel", "label": "拼豆像素", "state": "pending", "detail": "最近邻色号 · 52×52" }
  ]
}
```

`message`、`stages[].label`、`stages[].detail` 会直接出现在小程序上，只用工具说法（色块整理、正在整理色块、本地整理色块）。内部 `id` 仍是 `seedream`，不要把它或供应商、模型名写进这些字段。

`stages[].state`：`done` | `active` | `pending` | `fail`。

### `GET /api/v1/jobs/{id}`

只能看自己的任务。别人的任务 404。

`status`：`seedream` | `pixel` | `done` | `failed` | `cancelled`（提交后也可能短暂停在 `upload` / `queued`）。历史库里可能还有 `matting` / `cartoon`，新任务不会再进入。

完成后 `charged` 为 true，`balance` 是扣分后的余额，`draft` 是稿件详情。`original_url` / `cartoon_url` / `preview_url` 指向对象存储里的原图、Seedream 中间图和珠图。`cartoon_url` 这个字段名是旧列，内容是 Seedream（或开发模拟）的平涂图，不是人物动漫化。`matted_url` 新任务为 null。

失败时 HTTP 仍是 200，`status` 为 `failed`，`error` 说明原因，并写明本次未扣积分。`failed_stage` 会反映在对应步骤的 `state: "fail"` 上。

### `POST /api/v1/jobs/{id}/cancel`

进行中的任务改为 `cancelled`，文案「已取消，未扣积分」。已经 `done` 的任务保持完成态，不会把积分退回去。

珠图算法：把 Seedream 中间图最近邻缩放到用户选择的珠板（52、78 或 104），映射到内置拼豆色板（含 A1 / B3 / C2 / D1 / K9 等），再把超出色数上限的小色块并进最近色。不用扩散模型产生官方珠图。预览是一颗颗珠子的 PNG。

稿件详情额外字段：`pipeline`（`mvp_plus`；旧的官方模板是 `pixel`）、`provider`（`mock` | `seedream`；更早的稿可能是 `aliyun` 或 `local`）、`remove_bg`、`cartoon_strength`、`subject_type`、`palette_brand`、`pipeline_tags`、`pipeline_lines`、`original_url`、`matted_url`、`cartoon_url`、`source_key`。新稿的 `remove_bg` 为 false，`cartoon_strength` 为 `off`。列表卡片带 `pipeline_tags`，官方模板为空数组。展示用的标签和说明不写供应商或模型名，例如「色块整理」「本地色块」「拼豆色号」「最近邻色号」。失败文案同样不回传上游原文。

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

### `PUT /api/v1/drafts/{id}/pattern` · `PATCH /api/v1/drafts/{id}/pattern`

把改过的珠格写回这张稿。需要登录。小程序用 `PUT`。

```json
{ "cells_b64": "<标准 base64，行优先色板下标>", "save_as": false, "title": "可选，仅另存时用" }
```

- `cells_b64` 解码后是 `BYTEA`，每个字节是这张稿**原色板**里的下标，长度必须等于 `board_size * board_size`。下标超出色板返回 400「珠格色号超出色板」。长度不对返回 400「珠格数量和底板尺寸不一致」。不是合法 base64 返回 400「图纸数据有误」。
- 色板的 `code` / `name` / `hex` 保持不变，服务按新格子重算每色 `count`。`color_count` 是用量大于 0 的色数，`peg_count` 仍是整板颗数。用不到的颜色留在色板里、count 为 0，这样下标不会乱。
- 服务用现有预览算法重画 PNG，写入新的 `preview_key`。旧预览文件不删，出稿任务仍可能指着第一张图。
- 不改 `board_size`，不扣积分。
- `save_as` 省略或 `false`：覆盖自己的这张稿，返回更新后的详情。
- `save_as: true`：另存一张新稿（新 id），复制原图和出稿标记，格子和预览用改后的。`title` 有字时按改名规则校验（1–40 字）；不传则用「原标题 + 副本」，超长会先截断。原稿不动。
- 不是自己的稿：403「只能修改自己的稿件」。官方模板：403「官方模板不能修改」。不存在：404。未登录：401。

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

启动时不会写入官方预置稿，也不会写入作品或评论。「小橘猫吐舌」「黄油小熊」「柠檬汽水」「蜜桃兔耳」只作为上传示例图的默认标题。稿件库「官方」分类只返回库里仍标记为 `is_official` 的稿，新环境为空。教程是小程序内置只读内容，不读这些稿件。
