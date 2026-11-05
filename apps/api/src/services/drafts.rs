//! 稿件：出稿、列表、详情、改名、改格子、删除、收藏。

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::pixel;
use crate::services::seedream;
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
    pub pipeline_tags: Vec<String>,
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
    pub pipeline: String,
    pub provider: String,
    pub remove_bg: bool,
    pub cartoon_strength: String,
    pub subject_type: String,
    pub palette_brand: String,
    pub pipeline_tags: Vec<String>,
    pub pipeline_lines: Vec<String>,
    pub original_url: Option<String>,
    pub matted_url: Option<String>,
    pub cartoon_url: Option<String>,
    pub source_url: Option<String>,
    pub source_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeleteOutput {
    pub deleted: bool,
    /// 历史上若关联过作品行，删除稿件时会一并清掉。
    pub work_deleted: bool,
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
               d.pipeline, d.provider, d.subject_type,
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
               d.pipeline, d.provider, d.remove_bg, d.cartoon_strength, d.subject_type,
               d.palette_brand, d.matted_key, d.cartoon_key, d.source_key,
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

/// 把格子写回原稿，或另存为一张新稿。
///
/// `cells` 仍是行优先的色板下标（`BYTEA`）。色板沿用稿件原来的 code / name / hex，只重算 count。
/// 预览用新的对象键，避免小程序按旧地址命中缓存。旧预览文件留着：出稿任务可能还指着第一张图。
pub async fn update_pattern(
    state: &AppState,
    user_id: Uuid,
    draft_id: Uuid,
    cells_b64: &str,
    save_as: bool,
    title: Option<&str>,
) -> AppResult<DraftDetail> {
    gate_owner(&state.db, user_id, draft_id, "修改").await?;
    let row = sqlx::query_as::<_, PatternRow>(
        r#"
        SELECT title, board_size, palette, source_key, palette_pref,
               pipeline, provider, remove_bg, cartoon_strength, subject_type,
               palette_brand, matted_key, cartoon_key, pipeline_tags
        FROM drafts
        WHERE id = $1
        "#,
    )
    .bind(draft_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("稿件不存在".into()))?;

    let new_title = if save_as {
        Some(copy_title(title, &row.title)?)
    } else {
        None
    };
    let cells = decode_cells(cells_b64)?;
    let (palette, color_count, peg_count, pattern) =
        prepare_pattern(&row.palette, &cells, row.board_size)?;
    let png = pixel::render_preview_png(&pattern)?;
    let target_id = if save_as { Uuid::new_v4() } else { draft_id };
    let preview_key = format!("previews/edit/{target_id}/{}.png", Uuid::new_v4());
    state
        .storage
        .put_object(&preview_key, &png, "image/png")
        .await?;

    let written = if let Some(title) = new_title {
        insert_pattern_copy(
            state,
            user_id,
            target_id,
            &title,
            &row,
            &palette,
            &cells,
            color_count,
            peg_count,
            &preview_key,
        )
        .await
    } else {
        overwrite_pattern(
            state,
            user_id,
            draft_id,
            &palette,
            &cells,
            color_count,
            peg_count,
            &preview_key,
        )
        .await
    };
    if let Err(err) = written {
        let _ = state.storage.delete_object(&preview_key).await;
        return Err(err);
    }
    get_detail(state, user_id, target_id).await
}

async fn overwrite_pattern(
    state: &AppState,
    user_id: Uuid,
    draft_id: Uuid,
    palette: &serde_json::Value,
    cells: &[u8],
    color_count: i32,
    peg_count: i32,
    preview_key: &str,
) -> AppResult<()> {
    let updated = sqlx::query(
        r#"
        UPDATE drafts
        SET palette = $3,
            cells = $4,
            color_count = $5,
            peg_count = $6,
            preview_key = $7
        WHERE id = $1
          AND user_id = $2
          AND is_official = FALSE
          AND is_template = FALSE
        "#,
    )
    .bind(draft_id)
    .bind(user_id)
    .bind(palette)
    .bind(cells)
    .bind(color_count)
    .bind(peg_count)
    .bind(preview_key)
    .execute(&state.db)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound("稿件不存在".into()));
    }
    Ok(())
}

async fn insert_pattern_copy(
    state: &AppState,
    user_id: Uuid,
    draft_id: Uuid,
    title: &str,
    row: &PatternRow,
    palette: &serde_json::Value,
    cells: &[u8],
    color_count: i32,
    peg_count: i32,
    preview_key: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO drafts (
            id, user_id, title, board_size, color_count, peg_count,
            palette, cells, preview_key, source_key, palette_pref,
            is_template, is_official,
            pipeline, provider, remove_bg, cartoon_strength, subject_type,
            palette_brand, matted_key, cartoon_key, pipeline_tags
        )
        VALUES (
            $1,$2,$3,$4,$5,$6,
            $7,$8,$9,$10,$11,
            FALSE, FALSE,
            $12,$13,$14,$15,$16,$17,$18,$19,$20
        )
        "#,
    )
    .bind(draft_id)
    .bind(user_id)
    .bind(title)
    .bind(row.board_size)
    .bind(color_count)
    .bind(peg_count)
    .bind(palette)
    .bind(cells)
    .bind(preview_key)
    .bind(&row.source_key)
    .bind(&row.palette_pref)
    .bind(&row.pipeline)
    .bind(&row.provider)
    .bind(row.remove_bg)
    .bind(&row.cartoon_strength)
    .bind(&row.subject_type)
    .bind(&row.palette_brand)
    .bind(&row.matted_key)
    .bind(&row.cartoon_key)
    .bind(&row.pipeline_tags)
    .execute(&state.db)
    .await?;
    Ok(())
}

fn copy_title(explicit: Option<&str>, original: &str) -> AppResult<String> {
    if let Some(raw) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        return normalize_title(raw);
    }
    const SUFFIX: &str = " 副本";
    let room = 40usize.saturating_sub(SUFFIX.chars().count());
    let mut title: String = original.trim().chars().take(room).collect();
    title.push_str(SUFFIX);
    normalize_title(&title)
}

fn decode_cells(raw: &str) -> AppResult<Vec<u8>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("图纸数据有误".into()));
    }
    STANDARD
        .decode(trimmed)
        .map_err(|_| AppError::BadRequest("图纸数据有误".into()))
}

fn prepare_pattern(
    palette_json: &serde_json::Value,
    cells: &[u8],
    board_size: i32,
) -> AppResult<(serde_json::Value, i32, i32, pixel::Pattern)> {
    if !matches!(board_size, 52 | 78 | 104) {
        return Err(AppError::BadRequest("珠板尺寸仅支持 52、78、104".into()));
    }
    let side = board_size as usize;
    if cells.len() != side * side {
        return Err(AppError::BadRequest("珠格数量和底板尺寸不一致".into()));
    }
    let swatches = parse_palette_strict(palette_json)?;
    let mut counts = vec![0i32; swatches.len()];
    for &idx in cells {
        let slot = idx as usize;
        if slot >= swatches.len() {
            return Err(AppError::BadRequest("珠格色号超出色板".into()));
        }
        counts[slot] += 1;
    }
    let color_count = counts.iter().filter(|count| **count > 0).count() as i32;
    let peg_count = board_size * board_size;
    let palette = json!(swatches
        .iter()
        .enumerate()
        .map(|(index, swatch)| json!({
            "code": swatch.code,
            "name": swatch.name,
            "hex": swatch.hex,
            "count": counts[index],
        }))
        .collect::<Vec<_>>());
    let pattern = pixel::Pattern {
        width: board_size as u32,
        palette: swatches
            .iter()
            .enumerate()
            .map(|(index, swatch)| pixel::Swatch {
                code: swatch.code.clone(),
                name: swatch.name.clone(),
                hex: swatch.hex.clone(),
                rgb: swatch.rgb,
                count: counts[index],
            })
            .collect(),
        cells: cells.to_vec(),
    };
    Ok((palette, color_count, peg_count, pattern))
}

struct StoredSwatch {
    code: String,
    name: String,
    hex: String,
    rgb: [u8; 3],
}

fn parse_palette_strict(value: &serde_json::Value) -> AppResult<Vec<StoredSwatch>> {
    let Some(items) = value.as_array() else {
        return Err(AppError::BadRequest("色板无法读取".into()));
    };
    if items.is_empty() || items.len() > 256 {
        return Err(AppError::BadRequest("色板无法读取".into()));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let code = item
            .get("code")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let hex = item.get("hex").and_then(|v| v.as_str()).unwrap_or("");
        if code.is_empty() || name.is_empty() {
            return Err(AppError::BadRequest("色板无法读取".into()));
        }
        let Some(rgb) = parse_hex(hex) else {
            return Err(AppError::BadRequest("色板无法读取".into()));
        };
        out.push(StoredSwatch {
            code: code.to_string(),
            name: name.to_string(),
            hex: format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
            rgb,
        });
    }
    Ok(out)
}

fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some([r, g, b])
}

/// 删除自己的稿。
///
/// 历史上若关联过作品行，会在同一事务里先删掉（互动数据随外键级联），再删稿件。
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
    if let Some(key) = row.matted_key.as_deref() {
        let _ = state.storage.delete_object(key).await;
    }
    if let Some(key) = row.cartoon_key.as_deref() {
        let _ = state.storage.delete_object(key).await;
    }
    if let Some(key) = row.source_key.as_deref() {
        if key.starts_with("pipeline/") {
            let _ = state.storage.delete_object(key).await;
        }
    }
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
        SELECT user_id, is_official, is_template, preview_key, matted_key, cartoon_key, source_key
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

pub(crate) async fn load_image_bytes(
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

pub(crate) fn source_key_from_ref(refer: Option<&str>, public_base: &str) -> Option<String> {
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

pub(crate) fn pick_title(explicit: Option<&str>, refer: Option<&str>) -> AppResult<String> {
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
        pipeline_tags: seedream::pipeline_tags(&row.pipeline, &row.provider, &row.subject_type),
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
        pipeline: row.pipeline.clone(),
        provider: row.provider.clone(),
        remove_bg: row.remove_bg,
        cartoon_strength: row.cartoon_strength.clone(),
        subject_type: row.subject_type.clone(),
        palette_brand: row.palette_brand.clone(),
        pipeline_tags: seedream::pipeline_tags(&row.pipeline, &row.provider, &row.subject_type),
        pipeline_lines: seedream::pipeline_lines(
            &row.pipeline,
            &row.provider,
            &row.palette_pref,
            &row.palette_brand,
            row.board_size,
        ),
        original_url: row
            .source_key
            .as_deref()
            .map(|key| state.storage.public_url(key)),
        matted_url: row
            .matted_key
            .as_deref()
            .map(|key| state.storage.public_url(key)),
        cartoon_url: row
            .cartoon_key
            .as_deref()
            .map(|key| state.storage.public_url(key)),
        source_url: row
            .source_key
            .as_deref()
            .map(|key| state.storage.public_url(key)),
        source_key: row.source_key.clone(),
    }
}

#[derive(sqlx::FromRow)]
struct PatternRow {
    title: String,
    board_size: i32,
    palette: serde_json::Value,
    source_key: Option<String>,
    palette_pref: String,
    pipeline: String,
    provider: String,
    remove_bg: bool,
    cartoon_strength: String,
    subject_type: String,
    palette_brand: String,
    matted_key: Option<String>,
    cartoon_key: Option<String>,
    pipeline_tags: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct DraftGateRow {
    user_id: Uuid,
    is_official: bool,
    is_template: bool,
    preview_key: String,
    matted_key: Option<String>,
    cartoon_key: Option<String>,
    source_key: Option<String>,
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
    pipeline: String,
    provider: String,
    subject_type: String,
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
    pipeline: String,
    provider: String,
    remove_bg: bool,
    cartoon_strength: String,
    subject_type: String,
    palette_brand: String,
    matted_key: Option<String>,
    cartoon_key: Option<String>,
    source_key: Option<String>,
    author_nickname: String,
    favorited: bool,
    work_id: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> serde_json::Value {
        json!([
            {"code":"A1","name":"珊瑚","hex":"#FF6B5B","count":1},
            {"code":"W1","name":"米白","hex":"#f7f1ea","count":1}
        ])
    }

    #[test]
    fn pattern_edit_recounts_indexes_and_rejects_bad_cells() {
        let mut cells = vec![0u8; 52 * 52];
        cells[0] = 1;
        cells[8] = 1;
        let (edited, color_count, peg_count, pattern) =
            prepare_pattern(&palette(), &cells, 52).unwrap();
        assert_eq!(peg_count, 52 * 52);
        assert_eq!(color_count, 2);
        assert_eq!(edited[0]["count"].as_i64(), Some((52 * 52 - 2) as i64));
        assert_eq!(edited[1]["count"].as_i64(), Some(2));
        assert_eq!(edited[1]["hex"].as_str(), Some("#F7F1EA"));
        assert_eq!(pattern.cells.len(), 52 * 52);
        assert_eq!(pattern.palette[1].rgb, [0xF7, 0xF1, 0xEA]);
        let png = pixel::render_preview_png(&pattern).unwrap();
        assert!(png.starts_with(b"\x89PNG"));

        let short = vec![0u8; 10];
        let err = prepare_pattern(&palette(), &short, 52).unwrap_err();
        assert!(err.to_string().contains("不一致"));

        cells[1] = 9;
        let err = prepare_pattern(&palette(), &cells, 52).unwrap_err();
        assert!(err.to_string().contains("超出色板"));

        let mut wide = vec![0u8; 104 * 104];
        wide[3] = 1;
        let (_, colors, peg, _) = prepare_pattern(&palette(), &wide, 104).unwrap();
        assert_eq!(peg, 104 * 104);
        assert_eq!(colors, 2);

        assert!(decode_cells("").is_err());
        assert!(decode_cells("@@@").is_err());
        assert_eq!(decode_cells("AAE=").unwrap(), vec![0, 1]);

        let copied = copy_title(None, &"豆".repeat(40)).unwrap();
        assert_eq!(copied.chars().count(), 40);
        assert!(copied.ends_with("副本"));
        assert!(copy_title(Some("  "), "原稿").unwrap().ends_with("副本"));
        assert_eq!(copy_title(Some(" 窗边 "), "原稿").unwrap(), "窗边");
        assert!(copy_title(Some(&"豆".repeat(41)), "原稿").is_err());
    }
}
