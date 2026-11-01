# 拼豆（pindou）

微信小程序「拼豆」可运行 MVP：uni-app Vue3 + TypeScript + Pinia，Rust axum + PostgreSQL。

拍照或选一张图，按 52 / 78 / 104 珠板和色数生成色号稿。出稿时可以起名，之后能改名；删除自己的稿时，若已发布，广场作品会一起下架。积分规则是每日 +3、成功出稿 −1、激励广告 +1。广场可以点赞、收藏、评论。

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
| 每日领取 | +3（东八区每天一次，PostgreSQL `daily_claims` 幂等） |
| 成功出稿 | −1（整条 MVP+：去背 + 卡通 + 拼豆，只扣一次；失败或取消不扣，也不退款） |
| 激励广告 | +1 |
| 充值 | 10 / ¥6，35 / ¥18，60 / ¥30。未配商户参数时开发示意到账；配齐后走微信支付 JSAPI，验签成功才入账一次 |

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

进程启动时会执行 `migrations/` 并写入官方模板。默认听 `http://127.0.0.1:3000`。对象文件落在 `apps/api/data/objects`（已 gitignore）。

```bash
curl http://127.0.0.1:3000/health
curl -X POST http://127.0.0.1:3000/api/v1/auth/wx-login \
  -H 'content-type: application/json' \
  -d '{"code":"mock:demo"}'
```

把返回的 `token` 放到后面的 `Authorization: Bearer`。

微信真登录：在 `.env` 填 `WX_APPID` 和 `WX_SECRET`。留空则走开发态，同一 `code` 对应同一用户，不会请求微信。支付要用这个登录拿到的 `openid`，所以 AppID 必须和小程序、商户号是一套。

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

MVP+ 出稿默认「智能去背 → 人物动漫化 → 自研最近邻拼豆」。阿里云密钥见下方。没配密钥时走开发模拟，仍然会产出透明底、卡通中间图和真实色号稿，方便本地和 CI。

## 阿里云视觉（可选）

在 `apps/api/.env`：

| 变量 | 作用 |
|------|------|
| `ALIYUN_ACCESS_KEY_ID` | 访问密钥 ID。留空则不请求阿里云 |
| `ALIYUN_ACCESS_KEY_SECRET` | 访问密钥 Secret。不要提交到仓库 |
| `ALIYUN_REGION` | 默认 `cn-shanghai` |
| `PIPELINE_PROVIDER` | `auto`（有密钥用阿里云，否则模拟）/ `mock` / `aliyun` |
| `ALIYUN_IMAGESEG_ENDPOINT` | 可选，分割 API 主机名 |
| `ALIYUN_IMAGEENHAN_ENDPOINT` | 可选，动漫化 API 主机名 |
| `PUBLIC_BASE_URL` | 原图和抠图的公网地址。阿里云要能下载这个 URL |

人像走图像分割 `SegmentBody`，宠物和静物走 `SegmentCommonImage`。卡通走图像生产 `GenerateHumanAnimeStyle`（轻 = handdrawn，标准 = anime，强 = 3d）。珠图不用扩散模型，是最近邻缩到 52/78/104 再映射内置 SKU 色板。

`PIPELINE_PROVIDER=aliyun` 但没配密钥时，出稿接口直接返回 400，不会扣积分。

积分：整次成功才扣 1 分，写在落稿的同一个事务里。中间任何一步失败或用户取消，都不扣，因此也没有退款流水。

## 3. 测试

Postgres 要先起来（和 `cargo run` 同一套 `DATABASE_URL`）。

```bash
cd apps/api
cargo test
```

覆盖：开发态登录、日领 PostgreSQL 幂等、坏图不扣分、MVP+ 模拟管线（去背 / 卡通 / 最近邻色号，成功才扣 1 分）、改名删除、广场点赞评论、开发示意充值只入账一次、微信支付验签回调幂等、坏签名和未支付不加积分、余额不足 402。主流程强制 `PIPELINE_PROVIDER=mock`，并清掉商户参数，避免测试打到真微信。支付用例使用本机假微信和测试密钥。

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

建议走一遍：登录 → 领取今日 +3 → 出稿（拍照 / 相册 / 示例图）→ 选珠板、色数、去背和卡通强度 → 生成页看上传 / 去背 / 卡通 / 拼豆 → 预览里对比原图、卡通中间态和拼豆结果 → 稿件库看到管线标签 → 发布到广场 → 点赞评论。积分为 0 时会弹出「看广告 / 去充值」。没配阿里云时，生成页的去背说明是「开发模拟」。

## 5. 对象存储

`apps/api/src/services/storage.rs` 的本地实现返回 `key` / `url` / `etag` / `content_type`，和 COS PutObject 的使用方式对齐。换成 COS 时保持这几个字段即可。

## 接口

详见 [docs/api.md](docs/api.md)。

## License

私有项目，未开源授权前请勿外传。
