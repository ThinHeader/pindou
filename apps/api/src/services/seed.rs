//! 首次启动写入官方模板和广场示例，方便稿件库 / 广场不是空的。

use anyhow::Context;
use serde_json::json;
use uuid::Uuid;

use crate::services::pixel::{self, Pref};
use crate::state::AppState;

struct SeedItem {
    scene: &'static str,
    title: &'static str,
    caption: &'static str,
    category: &'static str,
    board: u32,
    colors: u32,
    author_openid: &'static str,
    author_name: &'static str,
    likes: i32,
    favorites: i32,
}

const ITEMS: &[SeedItem] = &[
    SeedItem {
        scene: "cat",
        title: "小橘猫吐舌",
        caption: "第一次试暖色系出稿，耳朵和舌头用了珊瑚粉，适合做钥匙扣。",
        category: "pet",
        board: 52,
        colors: 12,
        author_openid: "seed:youzi",
        author_name: "柚子酱",
        likes: 328,
        favorites: 96,
    },
    SeedItem {
        scene: "bear",
        title: "黄油小熊",
        caption: "奶黄和杏桃就够了，新手从眼睛开始铺。",
        category: "pet",
        board: 52,
        colors: 8,
        author_openid: "seed:bear",
        author_name: "小熊烘培",
        likes: 210,
        favorites: 40,
    },
    SeedItem {
        scene: "chick",
        title: "柠檬汽水",
        caption: "大板试试晴空色翅膀，挂在包上刚刚好。",
        category: "food",
        board: 104,
        colors: 18,
        author_openid: "seed:ningmeng",
        author_name: "柠檬汽",
        likes: 891,
        favorites: 120,
    },
    SeedItem {
        scene: "bunny",
        title: "蜜桃兔耳",
        caption: "马卡龙色兔子，耳尖用樱粉点一下。",
        category: "festival",
        board: 52,
        colors: 10,
        author_openid: "seed:meimei",
        author_name: "莓莓手作",
        likes: 562,
        favorites: 77,
    },
];

pub async fn seed_if_empty(state: &AppState) -> anyhow::Result<()> {
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(8675309)")
        .execute(&mut *tx)
        .await?;

    let official_id = ensure_user(&mut tx, "system:pindou", "拼豆官方").await?;
    let mut author_ids = Vec::new();
    for item in ITEMS {
        author_ids.push(ensure_user(&mut tx, item.author_openid, item.author_name).await?);
    }
    let comment_users = [
        ensure_user(&mut tx, "seed:taotao", "桃桃豆").await?,
        author_ids[1],
        author_ids[2],
    ];

    for (item, author_id) in ITEMS.iter().zip(author_ids.iter().copied()) {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM drafts WHERE is_official = TRUE AND title = $1)",
        )
        .bind(item.title)
        .fetch_one(&mut *tx)
        .await?;
        if exists {
            continue;
        }

        let src = pixel::scene_png(item.scene).map_err(|e| anyhow::anyhow!(e.message()))?;
        let pattern = pixel::pixelate(&src, item.board, item.colors, Pref::Auto)
            .map_err(|e| anyhow::anyhow!(e.message()))?;
        let preview =
            pixel::render_preview_png(&pattern).map_err(|e| anyhow::anyhow!(e.message()))?;
        let draft_id = Uuid::new_v4();
        let preview_key = format!("previews/{draft_id}.png");
        let source_key = format!("sources/{draft_id}.png");
        // 文件写入放在事务外会拉长锁；种子只跑一次，锁内写本地盘可以接受。
        state
            .storage
            .put_object(&preview_key, &preview, "image/png")
            .await
            .context("保存示例预览")?;
        state
            .storage
            .put_object(&source_key, &src, "image/png")
            .await
            .context("保存示例原图")?;

        let palette = json!(pattern
            .palette
            .iter()
            .map(|s| json!({
                "code": s.code,
                "name": s.name,
                "hex": s.hex,
                "count": s.count,
            }))
            .collect::<Vec<_>>());

        sqlx::query(
            r#"
            INSERT INTO drafts (
                id, user_id, title, board_size, color_count, peg_count,
                palette, cells, preview_key, source_key, palette_pref,
                is_template, is_official
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'auto',TRUE,TRUE)
            "#,
        )
        .bind(draft_id)
        .bind(official_id)
        .bind(item.title)
        .bind(item.board as i32)
        .bind(pattern.palette.len() as i32)
        .bind((item.board * item.board) as i32)
        .bind(&palette)
        .bind(&pattern.cells)
        .bind(&preview_key)
        .bind(&source_key)
        .execute(&mut *tx)
        .await?;

        let work_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO works (
                id, user_id, draft_id, title, caption, category,
                board_size, color_count, peg_count, preview_key,
                like_count, favorite_count, comment_count
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,0)
            "#,
        )
        .bind(work_id)
        .bind(author_id)
        .bind(draft_id)
        .bind(item.title)
        .bind(item.caption)
        .bind(item.category)
        .bind(item.board as i32)
        .bind(pattern.palette.len() as i32)
        .bind((item.board * item.board) as i32)
        .bind(&preview_key)
        .bind(item.likes)
        .bind(item.favorites)
        .execute(&mut *tx)
        .await?;

        if item.scene == "cat" {
            let comments = [
                (comment_users[0], "耳朵颜色绝了！求色号表～"),
                (comment_users[1], "照着做了一版，挂在包上超可爱"),
                (comment_users[2], "新手友好吗？打算周末开拼"),
            ];
            for (uid, text) in comments {
                sqlx::query(
                    "INSERT INTO work_comments (work_id, user_id, content) VALUES ($1, $2, $3)",
                )
                .bind(work_id)
                .bind(uid)
                .bind(text)
                .execute(&mut *tx)
                .await?;
            }
            sqlx::query("UPDATE works SET comment_count = 3 WHERE id = $1")
                .bind(work_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

async fn ensure_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    openid: &str,
    nickname: &str,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (openid, nickname)
        VALUES ($1, $2)
        ON CONFLICT (openid) DO UPDATE SET nickname = EXCLUDED.nickname
        RETURNING id
        "#,
    )
    .bind(openid)
    .bind(nickname)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO credit_accounts (user_id, balance) VALUES ($1, 0) ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(id)
}
