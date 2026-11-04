//! MVP+ 出稿：上传 → 火山方舟 Seedream Flash 图生图 → 最近邻拼豆量化。
//!
//! 方舟返回约 1K 平涂图。珠板格数只看用户选择的 52 / 78 / 104，
//! 不看提示词里的点阵说法，也不把 API 图尺寸当成珠图。
//!
//! `POST /drafts/generate` 立刻返回任务，小程序轮询 `GET /jobs/{id}`。
//! 积分只在整条管线成功、稿件写入的同一事务里扣 1 分。失败或取消不扣，也不退款。

use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::credits::{self, GENERATE_COST};
use crate::services::drafts::{self, DraftDetail};
use crate::services::pixel::{self, Brand, Pref};
use crate::services::seedream::{self, SeedreamError, SubjectKind};
use crate::state::AppState;

#[derive(Debug)]
pub struct GenerateInput {
    pub image_base64: Option<String>,
    pub image_ref: Option<String>,
    pub board_size: Option<u32>,
    pub board_label: Option<String>,
    pub color_count: Option<u32>,
    pub palette_pref: Option<String>,
    pub title: Option<String>,
    pub remove_bg: Option<bool>,
    pub cartoon_strength: Option<String>,
    pub subject_type: Option<String>,
    pub palette_brand: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StageView {
    pub id: String,
    pub label: String,
    pub state: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct JobView {
    pub job_id: Uuid,
    pub status: String,
    pub progress: i32,
    pub message: String,
    pub stages: Vec<StageView>,
    pub provider: String,
    pub remove_bg: bool,
    pub cartoon_strength: String,
    pub subject_type: String,
    pub palette_brand: String,
    pub palette_pref: String,
    pub board_size: i32,
    pub color_count: i32,
    pub original_url: Option<String>,
    pub matted_url: Option<String>,
    pub cartoon_url: Option<String>,
    pub preview_url: Option<String>,
    pub draft_id: Option<Uuid>,
    pub draft: Option<DraftDetail>,
    pub balance: Option<i32>,
    pub error: Option<String>,
    /// 只有 `done` 且已经写入扣减时为 true。
    pub charged: bool,
}

struct Fail {
    stage: String,
    message: String,
}

pub async fn enqueue(state: &AppState, user_id: Uuid, input: GenerateInput) -> AppResult<JobView> {
    let provider = state
        .config
        .provider_label()
        .map_err(|message| AppError::BadRequest(message.to_string()))?;

    let board = pixel::parse_board(input.board_size, input.board_label.as_deref())?;
    let color_cap = pixel::parse_color_count(input.color_count)?;
    let pref = Pref::parse(input.palette_pref.as_deref().unwrap_or("auto"))
        .ok_or_else(|| AppError::BadRequest("色系偏好无效".into()))?;
    // 去背和卡通强度是旧阿里云参数。仍接收，避免旧客户端直接 400，但不再影响出稿。
    let _legacy = (&input.remove_bg, &input.cartoon_strength);
    let subject = SubjectKind::parse(input.subject_type.as_deref().unwrap_or("person"))
        .ok_or_else(|| AppError::BadRequest("主体类型无效，请选择人像、宠物或静物".into()))?;
    let brand = Brand::parse(input.palette_brand.as_deref().unwrap_or("generic"))
        .ok_or_else(|| AppError::BadRequest("色盘品牌无效".into()))?;
    let title = drafts::pick_title(input.title.as_deref(), input.image_ref.as_deref())?;

    let bytes = drafts::load_image_bytes(
        state,
        input.image_base64.as_deref(),
        input.image_ref.as_deref(),
    )
    .await?;
    pixel::validate_image(&bytes)?;

    let snapshot = credits::view(state, user_id).await?;
    if snapshot.balance < GENERATE_COST {
        return Err(AppError::InsufficientCredits(format!(
            "积分不足：需要 {GENERATE_COST}，当前 {}",
            snapshot.balance
        )));
    }

    let job_id = Uuid::new_v4();
    let (original_key, owned) = persist_original(state, &input, job_id, &bytes).await?;
    let (status, progress, message) = ("seedream", 36, "正在整理色块");

    let inserted = sqlx::query(
        r#"
        INSERT INTO generation_jobs (
            id, user_id, status, progress, message,
            board_size, color_count, palette_pref, remove_bg, cartoon_strength,
            subject_type, palette_brand, title, provider, original_key
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
        "#,
    )
    .bind(job_id)
    .bind(user_id)
    .bind(status)
    .bind(progress)
    .bind(message)
    .bind(board as i32)
    .bind(color_cap as i32)
    .bind(pref.as_str())
    .bind(false)
    .bind("off")
    .bind(subject.as_str())
    .bind(brand.as_str())
    .bind(&title)
    .bind(provider)
    .bind(&original_key)
    .execute(&state.db)
    .await;

    if let Err(err) = inserted {
        if owned {
            let _ = state.storage.delete_object(&original_key).await;
        }
        return Err(err.into());
    }

    let worker = state.clone();
    tokio::spawn(async move {
        run(worker, job_id).await;
    });

    get_for_user(state, user_id, job_id).await
}

pub async fn get_for_user(state: &AppState, user_id: Uuid, job_id: Uuid) -> AppResult<JobView> {
    let row = load_job(state, job_id).await?;
    if row.user_id != user_id {
        return Err(AppError::NotFound("任务不存在".into()));
    }
    to_view(state, user_id, &row).await
}

pub async fn cancel(state: &AppState, user_id: Uuid, job_id: Uuid) -> AppResult<JobView> {
    sqlx::query(
        r#"
        UPDATE generation_jobs
        SET status = 'cancelled',
            failed_stage = status,
            message = '已取消，未扣积分',
            error_message = '已取消，未扣积分',
            updated_at = now()
        WHERE id = $1 AND user_id = $2
          AND status IN ('queued', 'upload', 'matting', 'cartoon', 'seedream', 'pixel')
        "#,
    )
    .bind(job_id)
    .bind(user_id)
    .execute(&state.db)
    .await?;
    get_for_user(state, user_id, job_id).await
}

async fn run(state: AppState, job_id: Uuid) {
    if let Err(fail) = run_inner(&state, job_id).await {
        tracing::info!(%job_id, stage = %fail.stage, message = %fail.message, "出稿管线未完成");
        let _ = mark_failed(&state, job_id, &fail.stage, &fail.message).await;
    }
}

async fn run_inner(state: &AppState, job_id: Uuid) -> Result<(), Fail> {
    let row = load_job(state, job_id).await.map_err(|err| Fail {
        stage: "upload".into(),
        message: no_charge(user_message(err)),
    })?;
    if is_terminal(&row.status) {
        return Ok(());
    }
    let original_key = row.original_key.clone().ok_or_else(|| Fail {
        stage: "upload".into(),
        message: "找不到参考图，请重新上传。本次未扣积分。".into(),
    })?;
    let pref = Pref::parse(&row.palette_pref).unwrap_or(Pref::Auto);
    let brand = Brand::parse(&row.palette_brand).unwrap_or(Brand::Generic);
    let source_bytes = read_object(state, &original_key, "upload").await?;

    if !advance(state, job_id, "seedream", 48, "正在整理色块").await? {
        return Ok(());
    }
    let stylized = match row.provider.as_str() {
        "mock" => seedream::mock_stylize(&source_bytes).map_err(|err| fail_vis("seedream", err))?,
        "seedream" => {
            let client = seedream::http_client().map_err(|err| Fail {
                stage: "seedream".into(),
                message: err.into_message(),
            })?;
            // 原图字节已在本地。reference_image 会内联 data URL，
            // 不把 PUBLIC_BASE_URL/files/... 交给方舟去拉。
            let image_url = state.storage.public_url(&original_key);
            seedream::run_ark(&client, &state.config, &image_url, &source_bytes)
                .await
                .map_err(|err| fail_vis("seedream", err))?
        }
        _ => {
            return Err(Fail {
                stage: "seedream".into(),
                message: "这条任务的出稿方式已停用，请重新出稿。本次未扣积分。".into(),
            });
        }
    };
    let stylized_key = format!("pipeline/{job_id}/seedream.png");
    put_png(state, &stylized_key, &stylized, "seedream").await?;
    if !set_key(state, job_id, "cartoon_key", &stylized_key).await? {
        return Ok(());
    }

    if !advance(state, job_id, "pixel", 82, "拼豆像素中").await? {
        return Ok(());
    }
    let pattern = pixel::pixelate_beads(
        &stylized,
        row.board_size as u32,
        row.color_count as u32,
        pref,
        brand,
    )
    .map_err(|err| Fail {
        stage: "pixel".into(),
        message: no_charge(err.message().into()),
    })?;
    let preview = pixel::render_preview_png(&pattern).map_err(|err| Fail {
        stage: "pixel".into(),
        message: no_charge(err.message().into()),
    })?;
    let preview_key = format!("previews/{job_id}.png");
    put_png(state, &preview_key, &preview, "pixel").await?;
    if !set_key(state, job_id, "preview_key", &preview_key).await? {
        let _ = state.storage.delete_object(&preview_key).await;
        return Ok(());
    }

    let actual = pattern.palette.len() as i32;
    let message = format!(
        "色号稿已生成（{}×{}，实际 {actual} 色）",
        row.board_size, row.board_size
    );
    let tags = seedream::pipeline_tags("mvp_plus", &row.provider, &row.subject_type);
    match finish(
        state,
        &row,
        &pattern,
        &original_key,
        None,
        &stylized_key,
        &preview_key,
        &message,
        &tags,
    )
    .await
    {
        Ok(Finish::Done) | Ok(Finish::Stopped) => {
            if matches!(finish_peek(state, job_id).await, Some(status) if status != "done") {
                let _ = state.storage.delete_object(&preview_key).await;
            }
            Ok(())
        }
        Err(fail) => {
            let _ = state.storage.delete_object(&preview_key).await;
            Err(fail)
        }
    }
}

enum Finish {
    Done,
    Stopped,
}

async fn finish_peek(state: &AppState, job_id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT status FROM generation_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
}

#[allow(clippy::too_many_arguments)]
async fn finish(
    state: &AppState,
    row: &JobRow,
    pattern: &pixel::Pattern,
    original_key: &str,
    matted_key: Option<String>,
    cartoon_key: &str,
    preview_key: &str,
    message: &str,
    tags: &[String],
) -> Result<Finish, Fail> {
    let palette = json!(pattern
        .palette
        .iter()
        .map(|swatch| json!({
            "code": swatch.code,
            "name": swatch.name,
            "hex": swatch.hex,
            "count": swatch.count,
        }))
        .collect::<Vec<_>>());
    let peg = row.board_size * row.board_size;
    let actual = pattern.palette.len() as i32;
    let tag_value = json!(tags);
    let matted_saved = matted_key;

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|err| db_fail("pixel", err))?;
    let status: String =
        sqlx::query_scalar("SELECT status FROM generation_jobs WHERE id = $1 FOR UPDATE")
            .bind(row.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|err| db_fail("pixel", err))?;
    if is_terminal(&status) {
        return Ok(Finish::Stopped);
    }
    let balance = credits::lock_balance(&mut tx, row.user_id)
        .await
        .map_err(|err| Fail {
            stage: "pixel".into(),
            message: no_charge(user_message(err)),
        })?;
    if balance < GENERATE_COST {
        return Err(Fail {
            stage: "pixel".into(),
            message: no_charge(format!("积分不足：需要 {GENERATE_COST}，当前 {balance}")),
        });
    }

    sqlx::query(
        r#"
        INSERT INTO drafts (
            id, user_id, title, board_size, color_count, peg_count,
            palette, cells, preview_key, source_key, palette_pref,
            is_template, is_official,
            remove_bg, cartoon_strength, subject_type, palette_brand,
            pipeline, provider, matted_key, cartoon_key, pipeline_tags
        )
        VALUES (
            $1,$2,$3,$4,$5,$6,
            $7,$8,$9,$10,$11,
            FALSE, FALSE,
            $12,$13,$14,$15,
            'mvp_plus',$16,$17,$18,$19
        )
        "#,
    )
    .bind(row.id)
    .bind(row.user_id)
    .bind(&row.title)
    .bind(row.board_size)
    .bind(actual)
    .bind(peg)
    .bind(&palette)
    .bind(&pattern.cells)
    .bind(preview_key)
    .bind(original_key)
    .bind(&row.palette_pref)
    .bind(row.remove_bg)
    .bind(&row.cartoon_strength)
    .bind(&row.subject_type)
    .bind(&row.palette_brand)
    .bind(&row.provider)
    .bind(&matted_saved)
    .bind(cartoon_key)
    .bind(&tag_value)
    .execute(&mut *tx)
    .await
    .map_err(|err| db_fail("pixel", err))?;

    let balance = credits::apply_delta_tx(
        &mut tx,
        row.user_id,
        -GENERATE_COST,
        "generate",
        Some(&row.id.to_string()),
    )
    .await
    .map_err(|err| Fail {
        stage: "pixel".into(),
        message: no_charge(user_message(err)),
    })?;

    sqlx::query(
        r#"
        UPDATE generation_jobs
        SET status = 'done',
            progress = 100,
            message = $2,
            draft_id = $3,
            balance_after = $4,
            preview_key = $5,
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(row.id)
    .bind(message)
    .bind(row.id)
    .bind(balance)
    .bind(preview_key)
    .execute(&mut *tx)
    .await
    .map_err(|err| db_fail("pixel", err))?;

    tx.commit().await.map_err(|err| db_fail("pixel", err))?;
    Ok(Finish::Done)
}

async fn persist_original(
    state: &AppState,
    input: &GenerateInput,
    job_id: Uuid,
    bytes: &[u8],
) -> AppResult<(String, bool)> {
    let has_b64 = input
        .image_base64
        .as_deref()
        .map(str::trim)
        .is_some_and(|raw| !raw.is_empty());
    if !has_b64 {
        if let Some(key) =
            drafts::source_key_from_ref(input.image_ref.as_deref(), &state.config.public_base_url)
        {
            state
                .storage
                .get_object(&key)
                .await
                .map_err(|err| match err {
                    AppError::NotFound(_) => {
                        AppError::BadRequest("找不到参考图，请重新上传".into())
                    }
                    other => other,
                })?;
            return Ok((key, false));
        }
    }
    let (ext, content_type) = sniff(bytes);
    let key = format!("pipeline/{job_id}/original.{ext}");
    state.storage.put_object(&key, bytes, content_type).await?;
    Ok((key, true))
}

fn sniff(bytes: &[u8]) -> (&'static str, &'static str) {
    if bytes.starts_with(b"\x89PNG") {
        ("png", "image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        ("jpg", "image/jpeg")
    } else if bytes.starts_with(b"GIF") {
        ("gif", "image/gif")
    } else {
        ("png", "image/png")
    }
}

async fn read_object(state: &AppState, key: &str, stage: &str) -> Result<Vec<u8>, Fail> {
    match state.storage.get_object(key).await {
        Ok((bytes, _)) => Ok(bytes),
        Err(err) => Err(Fail {
            stage: stage.into(),
            message: no_charge(user_message(err)),
        }),
    }
}

async fn put_png(state: &AppState, key: &str, bytes: &[u8], stage: &str) -> Result<(), Fail> {
    state
        .storage
        .put_object(key, bytes, "image/png")
        .await
        .map(|_| ())
        .map_err(|err| Fail {
            stage: stage.into(),
            message: no_charge(user_message(err)),
        })
}

async fn advance(
    state: &AppState,
    job_id: Uuid,
    status: &str,
    progress: i32,
    message: &str,
) -> Result<bool, Fail> {
    let updated = sqlx::query(
        r#"
        UPDATE generation_jobs
        SET status = $2, progress = $3, message = $4, updated_at = now()
        WHERE id = $1 AND status NOT IN ('cancelled', 'failed', 'done')
        "#,
    )
    .bind(job_id)
    .bind(status)
    .bind(progress)
    .bind(message)
    .execute(&state.db)
    .await
    .map_err(|err| db_fail(status, err))?;
    Ok(updated.rows_affected() > 0)
}

async fn set_key(state: &AppState, job_id: Uuid, column: &str, key: &str) -> Result<bool, Fail> {
    let sql = match column {
        "matted_key" => {
            "UPDATE generation_jobs SET matted_key = $2, updated_at = now() WHERE id = $1 AND status NOT IN ('cancelled', 'failed', 'done')"
        }
        "cartoon_key" => {
            "UPDATE generation_jobs SET cartoon_key = $2, updated_at = now() WHERE id = $1 AND status NOT IN ('cancelled', 'failed', 'done')"
        }
        "preview_key" => {
            "UPDATE generation_jobs SET preview_key = $2, updated_at = now() WHERE id = $1 AND status NOT IN ('cancelled', 'failed', 'done')"
        }
        _ => {
            return Err(Fail {
                stage: "pixel".into(),
                message: "出稿失败，请稍后重试。本次未扣积分。".into(),
            })
        }
    };
    let updated = sqlx::query(sql)
        .bind(job_id)
        .bind(key)
        .execute(&state.db)
        .await
        .map_err(|err| db_fail("pixel", err))?;
    Ok(updated.rows_affected() > 0)
}

async fn mark_failed(state: &AppState, job_id: Uuid, stage: &str, message: &str) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE generation_jobs
        SET status = 'failed',
            failed_stage = $2,
            error_message = $3,
            message = $3,
            updated_at = now()
        WHERE id = $1 AND status NOT IN ('done', 'cancelled', 'failed')
        "#,
    )
    .bind(job_id)
    .bind(stage)
    .bind(message)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn load_job(state: &AppState, job_id: Uuid) -> AppResult<JobRow> {
    sqlx::query_as::<_, JobRow>(
        r#"
        SELECT id, user_id, status, progress, message, error_message, failed_stage,
               board_size, color_count, palette_pref, remove_bg, cartoon_strength,
               subject_type, palette_brand, title, provider,
               original_key, matted_key, cartoon_key, preview_key,
               draft_id, balance_after
        FROM generation_jobs
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("任务不存在".into()))
}

async fn to_view(state: &AppState, user_id: Uuid, row: &JobRow) -> AppResult<JobView> {
    let draft = if row.status == "done" {
        if let Some(draft_id) = row.draft_id {
            drafts::get_detail(state, user_id, draft_id).await.ok()
        } else {
            None
        }
    } else {
        None
    };
    Ok(JobView {
        job_id: row.id,
        status: row.status.clone(),
        progress: row.progress,
        message: row.message.clone(),
        stages: build_stages(&StageSnap {
            status: row.status.clone(),
            failed_stage: row.failed_stage.clone(),
            provider: row.provider.clone(),
            board_size: row.board_size,
            error_message: row.error_message.clone(),
        }),
        provider: row.provider.clone(),
        remove_bg: row.remove_bg,
        cartoon_strength: row.cartoon_strength.clone(),
        subject_type: row.subject_type.clone(),
        palette_brand: row.palette_brand.clone(),
        palette_pref: row.palette_pref.clone(),
        board_size: row.board_size,
        color_count: row.color_count,
        original_url: row
            .original_key
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
        preview_url: row
            .preview_key
            .as_deref()
            .map(|key| state.storage.public_url(key)),
        draft_id: row.draft_id,
        draft,
        balance: row.balance_after,
        error: row.error_message.clone(),
        charged: row.status == "done" && row.balance_after.is_some(),
    })
}

struct StageSnap {
    status: String,
    failed_stage: Option<String>,
    provider: String,
    board_size: i32,
    error_message: Option<String>,
}

fn build_stages(row: &StageSnap) -> Vec<StageView> {
    const IDS: [(&str, &str); 3] = [
        ("upload", "上传原图"),
        ("seedream", "色块整理"),
        ("pixel", "拼豆像素"),
    ];
    let terminal_fail = matches!(row.status.as_str(), "failed" | "cancelled");
    let cursor = if row.status == "done" {
        IDS.len()
    } else if terminal_fail {
        row.failed_stage
            .as_deref()
            .and_then(stage_index)
            .unwrap_or(0)
    } else {
        stage_index(&row.status).unwrap_or(0)
    };

    IDS.iter()
        .enumerate()
        .map(|(index, (id, label))| {
            let state = if row.status == "done" {
                "done"
            } else if terminal_fail {
                if index < cursor {
                    "done"
                } else if index == cursor {
                    "fail"
                } else {
                    "pending"
                }
            } else if index < cursor {
                "done"
            } else if index == cursor {
                "active"
            } else {
                "pending"
            };
            StageView {
                id: (*id).to_string(),
                label: (*label).to_string(),
                state: state.to_string(),
                detail: stage_detail(id, state, row, terminal_fail),
            }
        })
        .collect()
}

fn stage_index(status: &str) -> Option<usize> {
    match status {
        "queued" | "upload" => Some(0),
        // 旧任务停在去背或卡通时，进度条落在现在的色块整理这一步。
        "matting" | "cartoon" | "seedream" => Some(1),
        "pixel" => Some(2),
        _ => None,
    }
}

fn stage_detail(id: &str, state: &str, row: &StageSnap, terminal_fail: bool) -> String {
    let mock = row.provider == "mock";
    let board = format!("{}×{}", row.board_size, row.board_size);
    match (id, state) {
        ("upload", "done") => "原图已就绪".into(),
        ("upload", "active") => "正在上传原图".into(),
        ("upload", "fail") => fail_detail(row),
        ("seedream", "active") => {
            if mock {
                "本地整理色块".into()
            } else {
                "正在整理色块".into()
            }
        }
        ("seedream", "done") => {
            if mock {
                "本地色块已整理".into()
            } else {
                "色块已整理".into()
            }
        }
        ("seedream", "fail") => fail_detail(row),
        ("seedream", "pending") => {
            if terminal_fail {
                "未开始".into()
            } else {
                "等待整理色块".into()
            }
        }
        ("pixel", "done") => format!("完成 · {board}"),
        ("pixel", "fail") => fail_detail(row),
        ("pixel", "pending") if terminal_fail => "未开始".into(),
        ("pixel", _) => format!("最近邻色号 · {board}"),
        _ => String::new(),
    }
}

fn fail_detail(row: &StageSnap) -> String {
    if row.status == "cancelled" {
        return "已取消，未扣积分".into();
    }
    row.error_message
        .clone()
        .filter(|msg| !msg.is_empty())
        .unwrap_or_else(|| "失败 · 网络或服务繁忙".into())
}

fn is_terminal(status: &str) -> bool {
    matches!(status, "done" | "failed" | "cancelled")
}

fn fail_vis(stage: &str, err: SeedreamError) -> Fail {
    Fail {
        stage: stage.into(),
        message: err.into_message(),
    }
}

fn db_fail(stage: &str, err: sqlx::Error) -> Fail {
    tracing::error!(error = %err, stage, "出稿任务写入失败");
    Fail {
        stage: stage.into(),
        message: "出稿失败，请稍后重试。本次未扣积分。".into(),
    }
}

fn user_message(err: AppError) -> String {
    match err {
        AppError::BadRequest(message)
        | AppError::Unauthorized(message)
        | AppError::Forbidden(message)
        | AppError::InsufficientCredits(message)
        | AppError::NotFound(message)
        | AppError::Gone(message)
        | AppError::ServiceUnavailable(message) => message,
        AppError::Conflict { message, .. } => message,
        AppError::Internal(err) => {
            tracing::error!(error = %err, "出稿内部错误");
            "出稿失败，请稍后重试".into()
        }
    }
}

fn no_charge(message: String) -> String {
    if message.contains("未扣积分") {
        message
    } else {
        format!("{message} 本次未扣积分。")
    }
}

#[derive(sqlx::FromRow)]
struct JobRow {
    id: Uuid,
    user_id: Uuid,
    status: String,
    progress: i32,
    message: String,
    error_message: Option<String>,
    failed_stage: Option<String>,
    board_size: i32,
    color_count: i32,
    palette_pref: String,
    remove_bg: bool,
    cartoon_strength: String,
    subject_type: String,
    palette_brand: String,
    title: String,
    provider: String,
    original_key: Option<String>,
    matted_key: Option<String>,
    cartoon_key: Option<String>,
    preview_key: Option<String>,
    draft_id: Option<Uuid>,
    balance_after: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(status: &str, failed: Option<&str>, board: i32) -> StageSnap {
        StageSnap {
            status: status.into(),
            failed_stage: failed.map(str::to_string),
            provider: "mock".into(),
            board_size: board,
            error_message: Some("出稿超时了。本次未扣积分。".into()),
        }
    }

    fn states(stages: &[StageView]) -> Vec<&str> {
        stages.iter().map(|stage| stage.state.as_str()).collect()
    }

    #[test]
    fn stages_follow_seedream_then_selected_board() {
        let stages = build_stages(&snap("seedream", None, 78));
        assert_eq!(states(&stages), ["done", "active", "pending"]);
        assert_eq!(stages[1].id, "seedream");
        assert_eq!(stages[1].label, "色块整理");
        assert_eq!(stages[1].detail, "本地整理色块");
        assert_eq!(stages[2].label, "拼豆像素");
        assert_eq!(stages[2].detail, "最近邻色号 · 78×78");
    }

    #[test]
    fn legacy_cartoon_status_lands_on_seedream_step() {
        let stages = build_stages(&snap("cartoon", None, 52));
        assert_eq!(states(&stages), ["done", "active", "pending"]);
        assert_eq!(stages[1].id, "seedream");
    }

    #[test]
    fn failure_marks_only_the_broken_stage() {
        let stages = build_stages(&snap("failed", Some("seedream"), 104));
        assert_eq!(states(&stages), ["done", "fail", "pending"]);
        assert!(stages[1].detail.contains("未扣积分"));
        assert_eq!(stages[2].detail, "未开始");
    }

    #[test]
    fn cancel_does_not_look_like_a_charge() {
        let stages = build_stages(&snap("cancelled", Some("pixel"), 52));
        assert_eq!(stages[2].state, "fail");
        assert_eq!(stages[2].detail, "已取消，未扣积分");
        assert_eq!(stages[0].state, "done");
    }
}
