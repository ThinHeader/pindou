# 拼豆（pindou）

微信小程序「拼豆」可运行 MVP：uni-app Vue3 + TypeScript + Pinia，Rust axum + PostgreSQL。

拍照或选一张图，按 52 / 78 / 104 珠板和色数生成色号稿；积分规则是每日 +3、成功出稿 −1、激励广告 +1。广场可以点赞、收藏、评论。

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
| 成功出稿 | −1（图片失败不扣） |
| 激励广告 | +1 |
| 充值 | 10 / ¥6，35 / ¥18，60 / ¥30（示意下单 + mock 回调） |

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

微信真登录：在 `.env` 填 `WX_APPID` 和 `WX_SECRET`。留空则走开发态，同一 `code` 对应同一用户，不会请求微信。

## 3. 测试

Postgres 要先起来（和 `cargo run` 同一套 `DATABASE_URL`）。

```bash
cd apps/api
cargo test
```

覆盖：开发态登录、日领 PostgreSQL 幂等、坏图不扣分、色号稿落库、广场点赞评论、示意充值回调、余额不足 402。

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

建议走一遍：登录 → 领取今日 +3 → 出稿（拍照 / 相册 / 示例图）→ 选 52·78·104 和色数 → 预览色号 → 稿件库 → 发布到广场 → 点赞评论。积分为 0 时会弹出「看广告 / 去充值」。

## 5. 对象存储

`apps/api/src/services/storage.rs` 的本地实现返回 `key` / `url` / `etag` / `content_type`，和 COS PutObject 的使用方式对齐。换成 COS 时保持这几个字段即可。

## 接口

详见 [docs/api.md](docs/api.md)。

## License

私有项目，未开源授权前请勿外传。
