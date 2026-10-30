# 拼豆（pindou）

微信小程序「拼豆」可运行工程骨架：uni-app Vue3 + TypeScript + Pinia 前端，Rust axum 后端，PostgreSQL + Redis 本地依赖。

视觉与页面流程对齐设计原型 **`/workspace/pindou-html`**（纯 HTML 高保真稿），本仓库是正式工程，**不会覆盖**该原型目录。

## 目录

```
pindou/
  README.md
  docker-compose.yml          # postgres:16 + redis:7
  apps/
    mini/                     # uni-app 小程序（手写最小结构）
    api/                      # Rust axum 后端
  packages/                   # 共享说明（可选）
  docs/
    api.md                    # 接口草图
```

## 积分规则

| 动作 | 积分 |
|------|------|
| 每日领取 | +3 |
| 成功出稿 | −1 |
| 激励广告 | +1 |
| 充值档位 | 占位（见 `docs/api.md`） |

## 1. 起本地数据库

需已安装 Docker。

```bash
cd pindou
docker compose up -d
```

- Postgres：`127.0.0.1:5432`，用户/密码/库均为 `pindou` / `pindou_dev` / `pindou`
- Redis：`127.0.0.1:6379`

环境变量模板：`apps/api/.env.example`（复制为 `apps/api/.env`）。

当前 API **stub 可不连库**即可启动；有库后再接 sqlx migrate。

## 2. 起 API

```bash
cd apps/api
cp .env.example .env   # 可选
cargo run
# 或仅检查：cargo check
```

默认监听 `http://127.0.0.1:3000`。

试一下：

```bash
curl http://127.0.0.1:3000/health
curl -X POST http://127.0.0.1:3000/api/v1/auth/wx-login -H 'content-type: application/json' -d '{"code":"demo"}'
curl http://127.0.0.1:3000/api/v1/credits/me
```

主要路由：

- `GET /health`
- `POST /api/v1/auth/wx-login`
- `GET /api/v1/credits/me`
- `POST /api/v1/credits/daily-claim`
- `POST /api/v1/credits/ad-reward`
- `POST /api/v1/drafts/generate`
- `GET /api/v1/plaza/feed`（stub）

## 3. 起小程序前端（mini）

`apps/mini` 为 **手写 uni-app Vue3 + Vite + TS 最小结构**，避免强依赖 `@dcloudio/uvm` 交互安装。

可选方式：

1. **HBuilderX**：打开 `apps/mini`，运行到微信开发者工具 / 浏览器。
2. **CLI（后续）**：在 `apps/mini` 执行 `npm i`，按需启用 `vite.config.ts` 里的 `@dcloudio/vite-plugin-uni`，再 `npm run dev:mp-weixin`。

API 基址：`src/api/http.ts` → `http://127.0.0.1:3000`。

UI：品牌色珊瑚红 `#FF6B5B`，tokens 见 `src/uni.scss`。可选 UI 库 `wot-design-uni`（`package.json` 中已注释说明）。

页面含：首页 / 广场 / 出稿 / 稿件库 / 我的（5 Tab），以及参数、预览、充值、稿件详情、作品详情、登录；积分不足用 `CreditEmptySheet` 弹层。

## 4. 与设计原型的关系

| 路径 | 用途 |
|------|------|
| `pindou-html/` | 静态高保真原型，浏览器直接打开 |
| `pindou/`（本仓库） | 可运行前后端骨架，接微信与真实服务 |

实现时以原型交互与视觉为准，接口以 `docs/api.md` 为准。

## 5. 对象存储

`apps/api/src/services/storage.rs` 为 COS/OSS 接口占位，本地 `DevStorage` stub。

## 开发说明

- 后端无 DB 时也应能 `cargo check`（sqlx 使用 runtime query，未强制编译期连库）。
- 微信登录未配置 `WX_APPID` 时返回演示用户 + 开发 JWT。
- 前端默认本地积分 store，可离线演示扣分 / 日领 / 广告；有 API 时优先打后端。

## License

私有项目，未开源授权前请勿外传。
