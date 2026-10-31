//! 稿件：出稿、列表、详情、改名、删除、收藏、发布到广场。

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::credits::{self, GENERATE_COST};
use crate::services::pixel::{self, Pref};
use crate::services::storage::{decode_image_base64, normalize_object_key};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct PaletteDto {
    pub code: String,
    pub name: String,
    pub hex: String,
    pub count: i32,
}

#[derive(Debug, Serialize)]
pub struct DraftCard {
    pub id: Uuid,
    pub title: String,
    pub board_size: i32,
    pub color_count: i32,
    pub peg_count: i32,
    pub preview_url: String,
    pub is_template: bool,
    pub is_official: bool,
    pub favorited: bool,
    pub badge: String,
    pub author_nickname: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct DraftDetail {
    pub id: Uuid,
    pub title: String,
    pub board_size: i32,
    pub color_count: i32,
    pub peg_count: i32,
    pub palette: Vec<PaletteDto>,
    pub cells_b64: String,
    pub preview_url: String,
    pub palette_pref: String,
    pub is_template: bool,
    pub is_official: bool,
    pub favorited: bool,
    pub badge: String,
    pub author_nickname: String,
    pub work_id: Option<Uuid>,
    pub hint: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct DeleteOutput {
    pub deleted: bool,
    /// 这张稿已经发布时，广场作品会一起删除。
    pub work_deleted: bool,
}

#[derive(Debug)]
pub struct GenerateInput {
    pub image_base64: Option<String>,
    pub image_ref: Option<String>,
    pub board_size: Option<u32>,
    pub board_label: Option<String>,
    pub color_count: Option<u32>,
    pub palette_pref: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GenerateOutput {
    pub job_id: Uuid,
    pub draft_id: Uuid,
    pub status: String,
    pub balance: i32,
    pub message: String,
    pub draft: DraftDetail,
}

pub async fn generate(
    state: &AppState,
    user_id: Uuid,
    input: GenerateInput,
) -> AppResult<GenerateOutput> {
    let board = pixel::parse_board(input.board_size, input.board_label.as_deref())?;
    let color_cap = pixel::parse_color_count(input.color_count)?;
    let pref = match input.palette_pref.as_deref().unwrap_or("auto") {
        raw => Pref::parse(raw).ok_or_else(|| AppError::BadRequest("色系偏好无效".into()))?,
    };
    // 标题不合法时直接拒绝，避免先算图再失败。空标题走默认名，不在这里报错。
    let title = pick_title(input.title.as_deref(), input.image_ref.as_deref())?;

    // 先看余额，图片失败时不会扣分；真正扣减仍在事务里再查一次。
    let snapshot = credits::view(state, user_id).await?;
    if snapshot.balance < GENERATE_COST {
        return Err(AppError::InsufficientCredits(format!(
            "积分不足：需要 {GENERATE_COST}，当前 {}",
            snapshot.balance
        )));
    }

    let bytes = load_image_bytes(
        state,
        input.image_base64.as_deref(),
        input.image_ref.as_deref(),
    )
    .await?;
    let pattern = pixel::pixelate(&bytes, board, color_cap, pref)?;
    let preview = pixel::render_preview_png(&pattern)?;
    let draft_id = Uuid::new_v4();
    let preview_key = format!("previews/{draft_id}.png");
    state
        .storage
        .put_object(&preview_key, &preview, "image/png")
        .await?;

    let source_key = source_key_from_ref(input.image_ref.as_deref(), &state.config.public_base_url);
    let palette_json = palette_value(&pattern);
    let actual_colors = pattern.palette.len() as i32;
    let peg = (board * board) as i32;

    let inserted = async {
        let mut tx = state.db.begin().await?;
        let balance = credits::lock_balance(&mut tx, user_id).await?;
        if balance < GENERATE_COST {
            return Err(AppError::InsufficientCredits(format!(
                "积分不足：需要 {GENERATE_COST}，当前 {balance}"
            )));
        }
        sqlx::query(
            r#"
            INSERT INTO drafts (
                id, user_id, title, board_size, color_count, peg_count,
                palette, cells, preview_key, source_key, palette_pref,
                is_template, is_official
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,FALSE,FALSE)
            "#,
        )
        .bind(draft_id)
        .bind(user_id)
        .bind(&title)
        .bind(board as i32)
        .bind(actual_colors)
        .bind(peg)
        .bind(&palette_json)
        .bind(&pattern.cells)
        .bind(&preview_key)
        .bind(&source_key)
        .bind(pref.as_str())
        .execute(&mut *tx)
        .await?;

        let balance = credits::apply_delta_tx(
            &mut tx,
            user_id,
            -GENERATE_COST,
            "generate",
            Some(&draft_id.to_string()),
        )
        .await?;
        tx.commit().await?;
        Ok::<i32, AppError>(balance)
    }
    .await;

    let balance = match inserted {
        Ok(b) => b,
        Err(err) => {
            let _ = state.storage.delete_object(&preview_key).await;
            return Err(err);
        }
    };

    let draft = get_detail(state, user_id, draft_id).await?;
    let message = format!("色号稿已生成（{board}×{board}，实际 {actual_colors} 色）");
    Ok(GenerateOutput {
        job_id: draft_id,
        draft_id,
        status: "ready".into(),
        balance,
        message,
        draft,
    })
}

pub async fn list(
    state: &AppState,
    user_id: Uuid,
    tab: &str,
    q: &str,
) -> AppResult<Vec<DraftCard>> {
    let tab = normalize_tab(tab)?;
    let like = like_pattern(q);
    let rows = sqlx::query_as::<_, DraftListRow>(
        r#"
        SELECT d.id, d.user_id, d.title, d.board_size, d.color_count, d.peg_count,
               d.preview_key, d.is_template, d.is_official, d.created_at,
               u.nickname AS author_nickname,
               EXISTS(
                 SELECT 1 FROM draft_favorites f
                 WHERE f.draft_id = d.id AND f.user_id = $1
               ) AS favorited
        FROM drafts d
        JOIN users u ON u.id = d.user_id
        WHERE (
            ($2 = 'mine' AND d.user_id = $1 AND d.is_official = FALSE)
            OR ($2 = 'favorite' AND EXISTS(
                SELECT 1 FROM draft_favorites f
                WHERE f.draft_id = d.id AND f.user_id = $1
            ))
            OR ($2 = 'template' AND d.is_template = TRUE)
            OR ($2 = 'official' AND d.is_official = TRUE)
            OR ($2 = 'all' AND (d.user_id = $1 OR d.is_official = TRUE))
        )
        AND ($3 = '' OR d.title ILIKE $3)
        ORDER BY d.created_at DESC
        LIMIT 60
        "#,
    )
    .bind(user_id)
    .bind(tab)
    .bind(&like)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| card_from_row(state, user_id, &row))
        .collect())
}

pub async fn get_detail(state: &AppState, user_id: Uuid, draft_id: Uuid) -> AppResult<DraftDetail> {
    let row = sqlx::query_as::<_, DraftDetailRow>(
        r#"
        SELECT d.id, d.user_id, d.title, d.board_size, d.color_count, d.peg_count,
               d.palette, d.cells, d.preview_key, d.palette_pref,
               d.is_template, d.is_official, d.created_at,
               u.nickname AS author_nickname,
               EXISTS(
                 SELECT 1 FROM draft_favorites f
                 WHERE f.draft_id = d.id AND f.user_id = $2
               ) AS favorited,
               (SELECT w.id FROM works w WHERE w.draft_id = d.id LIMIT 1) AS work_id
        FROM drafts d
        JOIN users u ON u.id = d.user_id
        WHERE d.id = $1
        "#,
    )
    .bind(draft_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("稿件不存在".into()))?;

    let visible = row.user_id == user_id || row.is_official || row.is_template || row.favorited;
    if !visible {
        return Err(AppError::NotFound("稿件不存在".into()));
    }
    Ok(detail_from_row(state, user_id, &row))
}

pub async fn toggle_favorite(pool: &PgPool, user_id: Uuid, draft_id: Uuid) -> AppResult<bool> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM drafts d
            WHERE d.id = $1 AND (d.user_id = $2 OR d.is_official OR d.is_template)
        )
        "#,
    )
    .bind(draft_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    if !exists {
        return Err(AppError::NotFound("稿件不存在".into()));
    }

    let inserted = sqlx::query(
        "INSERT INTO draft_favorites (user_id, draft_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(draft_id)
    .execute(pool)
    .await?;
    if inserted.rows_affected() > 0 {
        return Ok(true);
    }
    sqlx::query("DELETE FROM draft_favorites WHERE user_id = $1 AND draft_id = $2")
        .bind(user_id)
        .bind(draft_id)
        .execute(pool)
        .await?;
    Ok(false)
}

pub fn normalize_title(raw: &str) -> AppResult<String> {
    let title = raw.trim();
    let len = title.chars().count();
    if len == 0 {
        return Err(AppError::BadRequest("请填写稿件名称".into()));
    }
    if len > 40 {
        return Err(AppError::BadRequest("稿件名称请控制在 40 字以内".into()));
    }
    Ok(title.to_string())
}

pub async fn rename(
    state: &AppState,
    user_id: Uuid,
    draft_id: Uuid,
    title: &str,
) -> AppResult<DraftDetail> {
    let title = normalize_title(title)?;
    gate_owner(&state.db, user_id, draft_id, "修改").await?;
    let updated = sqlx::query("UPDATE drafts SET title = $3 WHERE id = $1 AND user_id = $2")
        .bind(draft_id)
        .bind(user_id)
        .bind(&title)
        .execute(&state.db)
        .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound("稿件不存在".into()));
    }
    get_detail(state, user_id, draft_id).await
}

/// 删除自己的稿。
///
/// 已发布的稿会连同广场作品一起删除（点赞、收藏、评论随作品外键级联）。
/// 没有单独的下架入口，只拒绝会让已发布稿永远删不掉。
/// 官方模板不删。预览图在事务提交后清理，上传原图保留。
pub async fn delete_owned(
    state: &AppState,
    user_id: Uuid,
    draft_id: Uuid,
) -> AppResult<DeleteOutput> {
    let row = gate_owner(&state.db, user_id, draft_id, "删除").await?;
    let mut tx = state.db.begin().await?;
    let works = sqlx::query("DELETE FROM works WHERE draft_id = $1")
        .bind(draft_id)
        .execute(&mut *tx)
        .await?;
    let gone = sqlx::query("DELETE FROM drafts WHERE id = $1 AND user_id = $2")
        .bind(draft_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    if gone.rows_affected() == 0 {
        return Err(AppError::NotFound("稿件不存在".into()));
    }
    tx.commit().await?;
    let _ = state.storage.delete_object(&row.preview_key).await;
    Ok(DeleteOutput {
        deleted: true,
        work_deleted: works.rows_affected() > 0,
    })
}

async fn gate_owner(
    pool: &PgPool,
    user_id: Uuid,
    draft_id: Uuid,
    verb: &str,
) -> AppResult<DraftGateRow> {
    let row = sqlx::query_as::<_, DraftGateRow>(
        r#"
        SELECT user_id, is_official, is_template, preview_key
        FROM drafts WHERE id = $1
        "#,
    )
    .bind(draft_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("稿件不存在".into()))?;

    if row.user_id != user_id {
        return Err(AppError::Forbidden(format!("只能{verb}自己的稿件")));
    }
    if row.is_official || row.is_template {
        return Err(AppError::Forbidden(format!("官方模板不能{verb}")));
    }
    Ok(row)
}

async fn load_image_bytes(
    state: &AppState,
    b64: Option<&str>,
    refer: Option<&str>,
) -> AppResult<Vec<u8>> {
    if let Some(raw) = b64.map(str::trim).filter(|s| !s.is_empty()) {
        return decode_image_base64(raw);
    }
    if let Some(refer) = refer.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(name) = refer.strip_prefix("builtin:") {
            return pixel::scene_png(name).map_err(Into::into);
        }
        let key = normalize_object_key(refer, &state.config.public_base_url);
        if key.is_empty() {
            return Err(AppError::BadRequest("请先选择图片".into()));
        }
        let (bytes, _) = state
            .storage
            .get_object(&key)
            .await
            .map_err(|err| match err {
                AppError::NotFound(_) => AppError::BadRequest("找不到参考图，请重新上传".into()),
                other => other,
            })?;
        return Ok(bytes);
    }
    Err(AppError::BadRequest("请先选择图片".into()))
}

fn source_key_from_ref(refer: Option<&str>, public_base: &str) -> Option<String> {
    let refer = refer.map(str::trim).filter(|s| !s.is_empty())?;
    if refer.starts_with("builtin:") {
        return None;
    }
    let key = normalize_object_key(refer, public_base);
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

fn pick_title(explicit: Option<&str>, refer: Option<&str>) -> AppResult<String> {
    if let Some(title) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        return normalize_title(title);
    }
    Ok(match refer.map(str::trim) {
        Some("builtin:cat") => "小橘猫吐舌".into(),
        Some("builtin:bear") => "黄油小熊".into(),
        Some("builtin:chick") => "柠檬汽水".into(),
        Some("builtin:bunny") => "蜜桃兔耳".into(),
        _ => "未命名草稿".into(),
    })
}

fn normalize_tab(tab: &str) -> AppResult<&'static str> {
    match tab.trim() {
        "" | "all" => Ok("all"),
        "mine" => Ok("mine"),
        "favorite" | "favorites" => Ok("favorite"),
        "template" | "templates" => Ok("template"),
        "official" => Ok("official"),
        _ => Err(AppError::BadRequest("稿件分类无效".into())),
    }
}

fn like_pattern(q: &str) -> String {
    let cleaned: String = q
        .trim()
        .chars()
        .filter(|c| *c != '%' && *c != '_' && *c != '\\')
        .take(40)
        .collect();
    if cleaned.is_empty() {
        String::new()
    } else {
        format!("%{cleaned}%")
    }
}

fn palette_value(pattern: &pixel::Pattern) -> serde_json::Value {
    json!(pattern
        .palette
        .iter()
        .map(|s| json!({
            "code": s.code,
            "name": s.name,
            "hex": s.hex,
            "count": s.count,
        }))
        .collect::<Vec<_>>())
}

fn palette_from_json(value: &serde_json::Value) -> Vec<PaletteDto> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(PaletteDto {
                        code: item.get("code")?.as_str()?.to_string(),
                        name: item.get("name")?.as_str()?.to_string(),
                        hex: item.get("hex")?.as_str()?.to_string(),
                        count: item.get("count")?.as_i64()? as i32,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn badge(is_official: bool, is_template: bool, favorited: bool, mine: bool) -> String {
    if is_official {
        "官方".into()
    } else if is_template {
        "模板".into()
    } else if favorited && !mine {
        "收藏".into()
    } else {
        "我的".into()
    }
}

fn card_from_row(state: &AppState, viewer: Uuid, row: &DraftListRow) -> DraftCard {
    DraftCard {
        id: row.id,
        title: row.title.clone(),
        board_size: row.board_size,
        color_count: row.color_count,
        peg_count: row.peg_count,
        preview_url: state.storage.public_url(&row.preview_key),
        is_template: row.is_template,
        is_official: row.is_official,
        favorited: row.favorited,
        badge: badge(
            row.is_official,
            row.is_template,
            row.favorited,
            row.user_id == viewer,
        ),
        author_nickname: row.author_nickname.clone(),
        created_at: row.created_at,
    }
}

fn detail_from_row(state: &AppState, viewer: Uuid, row: &DraftDetailRow) -> DraftDetail {
    let palette = palette_from_json(&row.palette);
    let top = palette.first().map(|p| p.code.as_str()).unwrap_or("主色");
    let minutes = (row.peg_count / 45).clamp(20, 180);
    DraftDetail {
        id: row.id,
        title: row.title.clone(),
        board_size: row.board_size,
        color_count: row.color_count,
        peg_count: row.peg_count,
        hint: format!("建议从用量最多的 {top} 开始往外铺。预计用时约 {minutes} 分钟。"),
        cells_b64: STANDARD.encode(&row.cells),
        palette,
        preview_url: state.storage.public_url(&row.preview_key),
        palette_pref: row.palette_pref.clone(),
        is_template: row.is_template,
        is_official: row.is_official,
        favorited: row.favorited,
        badge: badge(
            row.is_official,
            row.is_template,
            row.favorited,
            row.user_id == viewer,
        ),
        author_nickname: row.author_nickname.clone(),
        work_id: row.work_id,
        created_at: row.created_at,
    }
}

#[derive(sqlx::FromRow)]
struct DraftGateRow {
    user_id: Uuid,
    is_official: bool,
    is_template: bool,
    preview_key: String,
}

#[derive(sqlx::FromRow)]
struct DraftListRow {
    id: Uuid,
    user_id: Uuid,
    title: String,
    board_size: i32,
    color_count: i32,
    peg_count: i32,
    preview_key: String,
    is_template: bool,
    is_official: bool,
    created_at: DateTime<Utc>,
    author_nickname: String,
    favorited: bool,
}

#[derive(sqlx::FromRow)]
struct DraftDetailRow {
    id: Uuid,
    user_id: Uuid,
    title: String,
    board_size: i32,
    color_count: i32,
    peg_count: i32,
    palette: serde_json::Value,
    cells: Vec<u8>,
    preview_key: String,
    palette_pref: String,
    is_template: bool,
    is_official: bool,
    created_at: DateTime<Utc>,
    author_nickname: String,
    favorited: bool,
    work_id: Option<Uuid>,
}
