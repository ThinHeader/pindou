# 拼豆（pindou）

微信小程序「拼豆」可运行 MVP：uni-app Vue3 + TypeScript + Pinia，Rust axum + PostgreSQL。

拍照或选一张图，按 52 / 78 / 104 珠板和色数生成色号稿。出稿时可以起名，之后能改名。积分规则是成功出稿 −1、激励广告每天 +1。底部是首页、教程、出稿、稿件库、我的；教程是官方只读内容。作品发布、点赞和评论已关闭。

视觉对齐设计原型：品牌色 `#FF6B5B`，暖底、圆角卡片。原型 zip 只作对照，不进本仓库。

## 目录

```
README.md
docker-compose.yml          # postgres:16
rust-toolchain.toml         # Rust 1.99
apps/mini/                  # uni-app 小程序
apps/api/                   # axum API、SQL migration、集成测试
docs/api.md                 # 接口说明
```

## 积分

| 动作 | 积分 |
|------|------|
| 成功出稿 | −1（整条 MVP+：Seedream 出稿 + 拼豆，只扣一次；失败或取消不扣，也不退款） |
| 激励广告 | +1（东八区每天一次，看完 `uni.createRewardedVideoAd` 后才入账；`daily_claims` 幂等） |
| 充值 | 10 / ¥6，35 / ¥18，60 / ¥30。虚拟支付、商户 JSAPI、开发示意三者择一，都只入账一次 |

没有每日免费领取。广告位 ID 配在小程序 `VITE_REWARD_AD_UNIT_ID`（见 `apps/mini/.env.example`），留空时不会发积分。

## 1. 起数据库

需要 Docker。

```bash
docker compose up -d
```

- Postgres：`127.0.0.1:5432`，用户 / 密码 / 库 `pindou` / `pindou_dev` / `pindou`

没有 Docker 时，本机装同版本的 PostgreSQL 16，建同样的账号即可。

## 2. 起 API

工具链：Rust 1.99（仓库根目录有 `rust-toolchain.toml`，rustup 会自动装）。

```bash
cd apps/api
cp .env.example .env    # 按需改，不要提交 .env
cargo run
```

进程启动时会执行 `migrations/`，不会写入官方预置稿。默认听 `http://127.0.0.1:3000`。对象文件落在 `apps/api/data/objects`（已 gitignore）。

```bash
curl http://127.0.0.1:3000/health
curl -X POST http://127.0.0.1:3000/api/v1/auth/wx-login \
  -H 'content-type: application/json' \
  -d '{"code":"mock:demo"}'
```

把返回的 `token` 放到后面的 `Authorization: Bearer`。

微信真登录：在 `.env` 填 `WX_APPID` 和 `WX_SECRET`。留空则走开发态，同一 `code` 对应同一用户，不会请求微信。支付要用这个登录拿到的 `openid`。虚拟支付还要服务端保存的 `session_key`，所以必须先走一次微信登录；`session_key` 只以密文落库，不会返回给小程序。

## 虚拟支付（积分充值，优先）

个人主体小程序如果不能走商户号 JSAPI，可以用小程序虚拟支付（米大师道具直购）。配置了 `WX_VIRTUAL_OFFER_ID` 和对应环境的 AppKey 之后，`POST /api/v1/credits/orders` 返回 `virtual_payment`，小程序调用 `wx.requestVirtualPayment`。商户 JSAPI 代码仍保留：虚拟支付未配置时，行为与以前相同。

`GET /api/v1/credits/packs` 的 `pay_mode`：

| 值 | 含义 |
|----|------|
| `virtual` | 虚拟支付已启用 |
| `wechat` | 未启用虚拟支付，商户 JSAPI 已启用 |
| `mock` | 两者都没配。开发示意到账 |
| `misconfigured` | 虚拟支付或商户参数只写了一部分。示意到账关闭，下单 503 |

环境：

- 默认沙箱。`WX_VIRTUAL_ENV` 留空或 `1` 时，`signData.env` 为 `1`，AppKey 用 `WX_VIRTUAL_APPKEY_SANDBOX`。沙箱只对开发版、体验版生效，正式版会失败。
- 现网必须显式 `WX_VIRTUAL_ENV=0`，AppKey 用 `WX_VIRTUAL_APPKEY`（或 `WX_VIRTUAL_APPKEY_PROD`，两个都写时必须相同）。只配了现网 AppKey、但没把 env 设为 `0` 时，不会自动切到现网。
- 苹果支付不支持沙箱。

MP 后台（拼豆手账工坊，AppID `wx809eec952c85b197`）：

1. 开通虚拟支付。个人主体和企业主体的入口可能不同，以控制台实际页面为准。
2. 基本配置记下 OfferID，填到 `WX_VIRTUAL_OFFER_ID`。沙箱 AppKey 只放到部署环境的 `WX_VIRTUAL_APPKEY_SANDBOX`，不要写进仓库。
3. 道具管理创建三件道具，道具 ID 用 `small`、`popular`、`large`（或用 `WX_VIRTUAL_PRODUCT_*` 改成你的 ID）。价格分别是 600、1800、3000 分，并且发布到当前 env 对应的版本。道具大约 10 分钟后生效。
4. 开发管理 → 消息推送：URL 填 `https://你的域名/api/v1/pay/virtual/notify`（也就是 `{PUBLIC_BASE_URL}/api/v1/pay/virtual/notify`），数据格式 JSON，明文模式。Token 与 `WX_VIRTUAL_MSG_TOKEN` 一致。配了 Token 之后，服务会校验签名；推送字段与订单一致时入账。查单失败时，已验签的发货推送仍可入账。
5. 登录必须配置 `WX_APPID` 和 `WX_SECRET`。每次微信登录都会刷新 `session_key`。用户态签名失败时，小程序会提示重新登录。

支付成功后，微信推送 `xpay_goods_deliver_notify`。服务核对 openid、道具、金额和透传字段，再向微信查单。订单从 `created` 变成 `paid` 时写一条 `recharge` 流水，重复通知不加积分。客户端仍轮询 `GET /api/v1/credits/orders/{id}`；查到已支付会补入账，并调用 `notify_provide_goods` 告诉微信已发货。沙箱如果没有推送，轮询查单仍可以入账。

客诉退款不进小程序。配置 `OPS_REFUND_TOKEN` 后，用请求头 `X-Ops-Token` 调用 `POST /api/v1/ops/credit-orders/{订单UUID或商户单号}/refund`。服务按当前沙箱或现网 AppKey 调微信 `/xpay/refund_order`，确认退款完成后再扣回该档积分，重复调用不会再扣。用户登录 JWT 不能退款。沙箱待结算金额可以这样退回；虚拟支付后台需要具备退款能力。细节和 curl 见 `docs/api.md`。

变量名见 `apps/api/.env.example`。不要提交 AppKey 或退款口令。

## 微信支付（积分充值）

档位仍是小包 10 积分 / ¥6、推荐 35 / ¥18、大包 60 / ¥30。积分只加在原有 `credit_accounts` 上，没有第二套钱包。

**开发示意（默认）**：`WECHAT_MCH_ID`、私钥、证书序列号、APIv3 密钥、回调地址、微信支付公钥这些商户参数全部留空。`GET /api/v1/credits/packs` 的 `pay_mode` 为 `mock`。小程序会写明「开发示意充值」，确认后调用 `POST /api/v1/credits/orders/{id}/mock-callback`，同一订单只加一次积分，不会请求微信。

**正式支付**：上述参数配齐后，下单调用微信 API v3 `POST /v3/pay/transactions/jsapi`，把 `timeStamp` / `nonceStr` / `package` / `signType=RSA` / `paySign` 交给小程序 `uni.requestPayment`。用户取消或失败不加积分。微信把支付结果 POST 到 `WECHAT_PAY_NOTIFY_URL`（默认 `{PUBLIC_BASE_URL}/api/v1/pay/wechat/notify`）。服务校验平台签名、用 APIv3 密钥解密，核对金额、商户号、AppID 和付款人 `openid`，在同一事务里把订单从 `created` 改成 `paid` 并写一条 `recharge` 流水。`credit_orders.transaction_id` 和 `credit_ledger (reason, ref_id)` 有唯一约束，重复通知不会再加积分。客户端支付成功后会轮询 `GET /api/v1/credits/orders/{id}`；若回调稍慢，连的是官方微信域名时还会向微信查单并补入账。

只要填写了任意一项商户参数，示意回调就会拒绝，避免生产环境被假支付刷积分。参数不完整时下单返回 503，同样不加积分。

商户平台：

1. 小程序完成微信认证。登录用的 `WX_APPID` 和支付 AppID 是同一个。
2. 注册微信支付商户号，开通 **JSAPI 支付**。小程序支付走 JSAPI 下单，再由 `wx.requestPayment` 拉起。
3. **关联小程序**：商户平台 → 产品中心 → AppID 账号管理 → 关联小程序 AppID。
4. **JSAPI 支付授权目录**：这是公众号 / H5 网页支付要填的目录。小程序支付不使用 H5 授权目录，也不要填小程序页面路径。确认 JSAPI 产品已开通、AppID 已关联即可。如果开通页把授权目录标成必填，按商户平台当时的说明填写，它只对 H5 生效。
5. 账户中心 → API 安全：申请商户 API 证书（`apiclient_key.pem` + 证书序列号），设置 32 位 APIv3 密钥。推荐再申请「微信支付公钥」，把公钥 PEM 和公钥 ID 配进环境变量。不配公钥时，服务会请求 `/v3/certificates`，用 APIv3 密钥解密平台证书并缓存。
6. **支付回调 URL**：`https://你的域名/api/v1/pay/wechat/notify`。APIv3 把地址放在下单参数 `notify_url` 里，一般不用在商户平台另备案。必须是公网 HTTPS、不能带 query，微信服务器要能访问。`PUBLIC_BASE_URL` 指到这个域名，或单独设置 `WECHAT_PAY_NOTIFY_URL`。本地 http 不行，需要内网穿透。
7. 小程序后台把 API 域名加进 request 合法域名。`requestPayment` 由微信客户端发起，不走业务 request 域名。

变量说明见 `apps/api/.env.example`。不要提交私钥和 APIv3 密钥。

MVP+ 出稿默认「上传 → 火山方舟 Seedream 5.0 Flash 图生图 → 自研最近邻拼豆」。方舟密钥见下方。没配密钥时走开发模拟，仍然会产出透明底平涂中间图和真实色号稿，方便本地和 CI，也不会访问网络。

## 火山方舟 Seedream（出稿）

在 `apps/api/.env`：

| 变量 | 作用 |
|------|------|
| `ARK_API_KEY` | 火山方舟 API Key。留空则 `auto` 走开发模拟。不要提交到仓库 |
| `ARK_BASE_URL` | 可选，默认 `https://ark.cn-beijing.volces.com` |
| `ARK_MODEL` | 可选，默认 `doubao-seedream-5-0-flash-260915`（Seedream 5.0 Flash） |
| `PIPELINE_PROVIDER` | `auto`（有方舟密钥用 Seedream，否则模拟）/ `mock` / `seedream` / `ark` |
| `PUBLIC_BASE_URL` | 预览图、头像等对象的公网地址。Seedream 参考图不使用这个 URL，请求里内联 data URL |

这是方舟 OpenAPI `POST /api/v3/images/generations`（图生图），不是阿里云视觉智能，也不是 SeedEdit VIAPI。`ALIYUN_ACCESS_KEY_ID` / `ALIYUN_ACCESS_KEY_SECRET` 不能用于这条管线。`PIPELINE_PROVIDER=aliyun` 会直接 400。

Flash 大约 ¥0.12 / 张（约数）。MVP 只用 Flash，没有更高档的「增强」模型。接口通常返回约 1024×1024 的图。提示词里的点阵说法只是平涂风格，**珠板格数仍是用户选的 52 / 78 / 104**：Seedream 之后用最近邻缩到这块板，再映射内置 SKU 色板。不用扩散模型产生官方珠图。

`PIPELINE_PROVIDER=seedream` 或 `ark` 但没配 `ARK_API_KEY` 时，出稿接口直接返回 400，不会扣积分。

积分：整次成功才扣 1 分，写在落稿的同一个事务里。中间任何一步失败或用户取消，都不扣，因此也没有退款流水。

## 3. 测试

Postgres 要先起来（和 `cargo run` 同一套 `DATABASE_URL`）。

```bash
cd apps/api
cargo test
```

覆盖：开发态登录、激励广告每日一次、每日免费领取接口已移除、坏图不扣分、MVP+ 模拟管线（Seedream 风格的透明平涂中间图 / 最近邻色号，成功才扣 1 分）、52 / 78 / 104 都按所选珠板出格、没配方舟密钥时 `seedream` / `ark` / `aliyun` 返回 400 且不扣分、改名删除、作品发布与广场互动返回 410、开发示意充值只入账一次、微信支付验签回调幂等、虚拟支付签名与查单入账、客诉退款口令与扣回积分、坏签名和未支付不加积分、余额不足 402。主流程强制 `PIPELINE_PROVIDER=mock`，并清掉商户参数、虚拟支付参数和方舟密钥，避免测试打到真微信或真方舟。支付用例使用本机假微信和测试密钥。

## 4. 小程序

`apps/mini` 是 uni-app Vue3 + TS + Pinia，请求真实 HTTP。基址：

```bash
cd apps/mini
cp .env.example .env   # VITE_API_BASE=http://127.0.0.1:3000
npm install            # .npmrc 已打开 legacy-peer-deps，对齐 uni-app 与 Vue 3.4
```

微信开发者工具：

1. 用 HBuilderX 打开 `apps/mini`，运行到微信开发者工具；或 `npm run dev:mp-weixin`（需装好 `@dcloudio/vite-plugin-uni`）。
2. 模拟器可访问 `127.0.0.1`。真机改成电脑局域网 IP，并在开发者工具关闭「校验合法域名」（`manifest.json` 里 `urlCheck` 已是 false）。
3. 开发态不要设 `VITE_WX_LOGIN`。登录页「微信一键登录」会使用本机保存的稳定 mock code。后端配好 AppId/Secret 后，再把 `VITE_WX_LOGIN=1`，才会把 `uni.login` 的 code 交给 code2session。

建议走一遍：登录 → 出稿（拍照 / 相册 / 示例图）→ 选珠板和色数 → 生成页看上传 / 色块整理 / 拼豆像素 → 预览里对比原图、色块图和拼豆结果 → 稿件库看到处理标签 → 打开教程只读页。积分为 0 时会弹出「看广告 / 去充值」。看广告要配 `VITE_REWARD_AD_UNIT_ID`，并且在微信里把激励视频看完才会 +1，同一天再看不会再发。没配出稿密钥时，生成页的色块整理说明是「本地整理色块」。小程序界面不出现供应商或模型名。

## 5. 对象存储

`apps/api/src/services/storage.rs` 的本地实现返回 `key` / `url` / `etag` / `content_type`，和 COS PutObject 的使用方式对齐。换成 COS 时保持这几个字段即可。

## 接口

详见 [docs/api.md](docs/api.md)。

## License

私有项目，未开源授权前请勿外传。
