//! 首次启动写入官方模板，方便稿件库里有可参考的成品稿。

use anyhow::Context;
use serde_json::json;
use uuid::Uuid;

use crate::services::pixel::{self, Pref};
use crate::state::AppState;

struct SeedItem {
    scene: &'static str,
    title: &'static str,
    board: u32,
    colors: u32,
}

const ITEMS: &[SeedItem] = &[
    SeedItem {
        scene: "cat",
        title: "小橘猫吐舌",
        board: 52,
        colors: 12,
    },
    SeedItem {
        scene: "bear",
        title: "黄油小熊",
        board: 52,
        colors: 8,
    },
    SeedItem {
        scene: "chick",
        title: "柠檬汽水",
        board: 104,
        colors: 18,
    },
    SeedItem {
        scene: "bunny",
        title: "蜜桃兔耳",
        board: 52,
        colors: 10,
    },
];

pub async fn seed_if_empty(state: &AppState) -> anyhow::Result<()> {
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(8675309)")
        .execute(&mut *tx)
        .await?;

    let official_id = ensure_user(&mut tx, "system:pindou", "拼豆官方").await?;

    for item in ITEMS {
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
