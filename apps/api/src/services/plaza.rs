//! 广场：作品流、点赞、收藏、评论。

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct AuthorDto {
    pub id: Uuid,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WorkCard {
    pub id: Uuid,
    pub title: String,
    pub caption: String,
    pub category: String,
    pub board_size: i32,
    pub color_count: i32,
    pub peg_count: i32,
    pub preview_url: String,
    pub like_count: i32,
    pub favorite_count: i32,
    pub comment_count: i32,
    pub liked: bool,
    pub favorited: bool,
    pub author: AuthorDto,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CommentDto {
    pub id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub author: AuthorDto,
}

#[derive(Debug, Serialize)]
pub struct FeedPage {
    pub items: Vec<WorkCard>,
    pub next_cursor: Option<String>,
}

pub async fn publish(
    state: &AppState,
    user_id: Uuid,
    draft_id: Uuid,
    title: Option<String>,
    caption: Option<String>,
    category: Option<String>,
) -> AppResult<WorkCard> {
    let category = normalize_category(category.as_deref())?;
    let caption = trim_chars(caption.as_deref().unwrap_or(""), 200);

    if let Some(existing) = find_by_draft(&state.db, draft_id).await? {
        if existing.user_id != user_id {
            return Err(AppError::Forbidden("这张稿已经由别人发布".into()));
        }
        return get_work(state, Some(user_id), existing.id).await;
    }

    let draft = sqlx::query_as::<_, DraftPubRow>(
        r#"
        SELECT id, user_id, title, board_size, color_count, peg_count, preview_key, is_official
        FROM drafts WHERE id = $1
        "#,
    )
    .bind(draft_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("稿件不存在".into()))?;

    if draft.user_id != user_id {
        return Err(AppError::Forbidden("只能发布自己的稿件".into()));
    }

    let explicit = match title.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => Some(crate::services::drafts::normalize_title(raw)?),
        None => None,
    };
    // 带了标题就写回稿件，避免稿件库和「继续拼」仍显示「未命名草稿」。
    let rename_draft = explicit.as_ref().is_some_and(|next| next != &draft.title);
    let title = explicit.clone().unwrap_or_else(|| draft.title.clone());

    let work_id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    if rename_draft {
        sqlx::query("UPDATE drafts SET title = $2 WHERE id = $1 AND user_id = $3")
            .bind(draft_id)
            .bind(&title)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        r#"
        INSERT INTO works (
            id, user_id, draft_id, title, caption, category,
            board_size, color_count, peg_count, preview_key
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
        "#,
    )
    .bind(work_id)
    .bind(user_id)
    .bind(draft_id)
    .bind(&title)
    .bind(&caption)
    .bind(category)
    .bind(draft.board_size)
    .bind(draft.color_count)
    .bind(draft.peg_count)
    .bind(&draft.preview_key)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    get_work(state, Some(user_id), work_id).await
}

pub async fn feed(
    state: &AppState,
    viewer: Option<Uuid>,
    tab: &str,
    q: &str,
    cursor: Option<&str>,
    limit: i64,
) -> AppResult<FeedPage> {
    let tab = normalize_tab(tab)?;
    let limit = limit.clamp(1, 50);
    let like = like_pattern(q);
    let fetch = limit + 1;

    let rows = if tab == "hot" {
        let (likes, ts, id) = cursor.map(decode_hot_cursor).transpose()?.unwrap_or((
            i32::MAX,
            far_future(),
            Uuid::from_u128(u128::MAX),
        ));
        sqlx::query_as::<_, WorkRow>(
            r#"
            SELECT w.id, w.user_id, w.title, w.caption, w.category,
                   w.board_size, w.color_count, w.peg_count, w.preview_key,
                   w.like_count, w.favorite_count, w.comment_count, w.created_at,
                   u.nickname, u.avatar_url,
                   CASE WHEN $1::uuid IS NULL THEN FALSE ELSE EXISTS(
                       SELECT 1 FROM work_likes l WHERE l.work_id = w.id AND l.user_id = $1
                   ) END AS liked,
                   CASE WHEN $1::uuid IS NULL THEN FALSE ELSE EXISTS(
                       SELECT 1 FROM work_favorites f WHERE f.work_id = w.id AND f.user_id = $1
                   ) END AS favorited
            FROM works w
            JOIN users u ON u.id = w.user_id
            WHERE ($2 = '' OR w.title ILIKE $2)
              AND (w.like_count, w.created_at, w.id) < ($3, $4, $5)
            ORDER BY w.like_count DESC, w.created_at DESC, w.id DESC
            LIMIT $6
            "#,
        )
        .bind(viewer)
        .bind(&like)
        .bind(likes)
        .bind(ts)
        .bind(id)
        .bind(fetch)
        .fetch_all(&state.db)
        .await?
    } else {
        let category: Option<&str> = if tab == "recommend" { None } else { Some(tab) };
        let (ts, id) = cursor
            .map(decode_time_cursor)
            .transpose()?
            .unwrap_or((far_future(), Uuid::from_u128(u128::MAX)));
        sqlx::query_as::<_, WorkRow>(
            r#"
            SELECT w.id, w.user_id, w.title, w.caption, w.category,
                   w.board_size, w.color_count, w.peg_count, w.preview_key,
                   w.like_count, w.favorite_count, w.comment_count, w.created_at,
                   u.nickname, u.avatar_url,
                   CASE WHEN $1::uuid IS NULL THEN FALSE ELSE EXISTS(
                       SELECT 1 FROM work_likes l WHERE l.work_id = w.id AND l.user_id = $1
                   ) END AS liked,
                   CASE WHEN $1::uuid IS NULL THEN FALSE ELSE EXISTS(
                       SELECT 1 FROM work_favorites f WHERE f.work_id = w.id AND f.user_id = $1
                   ) END AS favorited
            FROM works w
            JOIN users u ON u.id = w.user_id
            WHERE ($2::text IS NULL OR w.category = $2)
              AND ($3 = '' OR w.title ILIKE $3)
              AND (w.created_at, w.id) < ($4, $5)
            ORDER BY w.created_at DESC, w.id DESC
            LIMIT $6
            "#,
        )
        .bind(viewer)
        .bind(category)
        .bind(&like)
        .bind(ts)
        .bind(id)
        .bind(fetch)
        .fetch_all(&state.db)
        .await?
    };

    let mut items: Vec<_> = rows.into_iter().map(|row| card_from(state, row)).collect();
    let next_cursor = if items.len() as i64 > limit {
        items.pop();
        items.last().map(|item| {
            if tab == "hot" {
                encode_hot_cursor(item.like_count, item.created_at, item.id)
            } else {
                encode_time_cursor(item.created_at, item.id)
            }
        })
    } else {
        None
    };
    Ok(FeedPage { items, next_cursor })
}

pub async fn get_work(
    state: &AppState,
    viewer: Option<Uuid>,
    work_id: Uuid,
) -> AppResult<WorkCard> {
    let row = sqlx::query_as::<_, WorkRow>(
        r#"
        SELECT w.id, w.user_id, w.title, w.caption, w.category,
               w.board_size, w.color_count, w.peg_count, w.preview_key,
               w.like_count, w.favorite_count, w.comment_count, w.created_at,
               u.nickname, u.avatar_url,
               CASE WHEN $2::uuid IS NULL THEN FALSE ELSE EXISTS(
                   SELECT 1 FROM work_likes l WHERE l.work_id = w.id AND l.user_id = $2
               ) END AS liked,
               CASE WHEN $2::uuid IS NULL THEN FALSE ELSE EXISTS(
                   SELECT 1 FROM work_favorites f WHERE f.work_id = w.id AND f.user_id = $2
               ) END AS favorited
        FROM works w
        JOIN users u ON u.id = w.user_id
        WHERE w.id = $1
        "#,
    )
    .bind(work_id)
    .bind(viewer)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("作品不存在".into()))?;
    Ok(card_from(state, row))
}

pub async fn toggle_like(state: &AppState, user_id: Uuid, work_id: Uuid) -> AppResult<(bool, i32)> {
    toggle_flag(state, user_id, work_id, true).await
}

pub async fn toggle_favorite(
    state: &AppState,
    user_id: Uuid,
    work_id: Uuid,
) -> AppResult<(bool, i32)> {
    toggle_flag(state, user_id, work_id, false).await
}

async fn toggle_flag(
    state: &AppState,
    user_id: Uuid,
    work_id: Uuid,
    is_like: bool,
) -> AppResult<(bool, i32)> {
    let mut tx = state.db.begin().await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM works WHERE id = $1)")
        .bind(work_id)
        .fetch_one(&mut *tx)
        .await?;
    if !exists {
        return Err(AppError::NotFound("作品不存在".into()));
    }

    let (table, column) = if is_like {
        ("work_likes", "like_count")
    } else {
        ("work_favorites", "favorite_count")
    };

    let inserted = sqlx::query(&format!(
        "INSERT INTO {table} (user_id, work_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
    ))
    .bind(user_id)
    .bind(work_id)
    .execute(&mut *tx)
    .await?;

    let on = if inserted.rows_affected() > 0 {
        true
    } else {
        sqlx::query(&format!(
            "DELETE FROM {table} WHERE user_id = $1 AND work_id = $2"
        ))
        .bind(user_id)
        .bind(work_id)
        .execute(&mut *tx)
        .await?;
        false
    };
    let delta = if on { 1 } else { -1 };
    let count: i32 = sqlx::query_scalar(&format!(
        "UPDATE works SET {column} = GREATEST({column} + $2, 0) WHERE id = $1 RETURNING {column}"
    ))
    .bind(work_id)
    .bind(delta)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((on, count))
}

pub async fn list_comments(state: &AppState, work_id: Uuid) -> AppResult<Vec<CommentDto>> {
    ensure_work(&state.db, work_id).await?;
    let rows = sqlx::query_as::<_, CommentRow>(
        r#"
        SELECT c.id, c.content, c.created_at, u.id AS author_id, u.nickname, u.avatar_url
        FROM work_comments c
        JOIN users u ON u.id = c.user_id
        WHERE c.work_id = $1
        ORDER BY c.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(work_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows.into_iter().map(comment_from).collect())
}

pub async fn add_comment(
    state: &AppState,
    user_id: Uuid,
    work_id: Uuid,
    content: &str,
) -> AppResult<CommentDto> {
    let content = content.trim();
    let len = content.chars().count();
    if !(1..=200).contains(&len) {
        return Err(AppError::BadRequest("评论需要 1 到 200 个字".into()));
    }
    let mut tx = state.db.begin().await?;
    ensure_work_tx(&mut tx, work_id).await?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO work_comments (id, work_id, user_id, content) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(work_id)
    .bind(user_id)
    .bind(content)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE works SET comment_count = comment_count + 1 WHERE id = $1")
        .bind(work_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let rows = list_comments(state, work_id).await?;
    rows.into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("评论写入后读取失败")))
}

pub async fn works_by_user(state: &AppState, user_id: Uuid) -> AppResult<Vec<WorkCard>> {
    let rows = sqlx::query_as::<_, WorkRow>(
        r#"
        SELECT w.id, w.user_id, w.title, w.caption, w.category,
               w.board_size, w.color_count, w.peg_count, w.preview_key,
               w.like_count, w.favorite_count, w.comment_count, w.created_at,
               u.nickname, u.avatar_url,
               EXISTS(SELECT 1 FROM work_likes l WHERE l.work_id = w.id AND l.user_id = $1) AS liked,
               EXISTS(SELECT 1 FROM work_favorites f WHERE f.work_id = w.id AND f.user_id = $1) AS favorited
        FROM works w
        JOIN users u ON u.id = w.user_id
        WHERE w.user_id = $1
        ORDER BY w.created_at DESC
        LIMIT 12
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows.into_iter().map(|row| card_from(state, row)).collect())
}

fn far_future() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("9999-12-31T00:00:00Z")
        .expect("cursor sentinel")
        .with_timezone(&Utc)
}

async fn ensure_work(pool: &sqlx::PgPool, work_id: Uuid) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM works WHERE id = $1)")
        .bind(work_id)
        .fetch_one(pool)
        .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound("作品不存在".into()))
    }
}

async fn ensure_work_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_id: Uuid,
) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM works WHERE id = $1)")
        .bind(work_id)
        .fetch_one(&mut **tx)
        .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound("作品不存在".into()))
    }
}

async fn find_by_draft(pool: &sqlx::PgPool, draft_id: Uuid) -> AppResult<Option<DraftOwner>> {
    let row = sqlx::query_as::<_, DraftOwner>("SELECT id, user_id FROM works WHERE draft_id = $1")
        .bind(draft_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

fn card_from(state: &AppState, row: WorkRow) -> WorkCard {
    WorkCard {
        id: row.id,
        title: row.title,
        caption: row.caption,
        category: row.category,
        board_size: row.board_size,
        color_count: row.color_count,
        peg_count: row.peg_count,
        preview_url: state.storage.public_url(&row.preview_key),
        like_count: row.like_count,
        favorite_count: row.favorite_count,
        comment_count: row.comment_count,
        liked: row.liked,
        favorited: row.favorited,
        author: AuthorDto {
            id: row.user_id,
            nickname: row.nickname,
            avatar_url: row.avatar_url,
        },
        created_at: row.created_at,
    }
}

fn comment_from(row: CommentRow) -> CommentDto {
    CommentDto {
        id: row.id,
        content: row.content,
        created_at: row.created_at,
        author: AuthorDto {
            id: row.author_id,
            nickname: row.nickname,
            avatar_url: row.avatar_url,
        },
    }
}

pub fn normalize_category(raw: Option<&str>) -> AppResult<&'static str> {
    match raw.unwrap_or("pet").trim() {
        "" | "pet" | "萌宠" => Ok("pet"),
        "food" | "食物" => Ok("food"),
        "pixel" | "像素人" => Ok("pixel"),
        "festival" | "节日" => Ok("festival"),
        "simple" | "简约" => Ok("simple"),
        _ => Err(AppError::BadRequest("作品分类无效".into())),
    }
}

fn normalize_tab(tab: &str) -> AppResult<&'static str> {
    match tab.trim() {
        "" | "recommend" | "推荐" => Ok("recommend"),
        "hot" | "热门" => Ok("hot"),
        other => normalize_category(Some(other)),
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

fn trim_chars(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

fn encode_time_cursor(ts: DateTime<Utc>, id: Uuid) -> String {
    format!("t:{},{id}", ts.to_rfc3339())
}

fn decode_time_cursor(raw: &str) -> AppResult<(DateTime<Utc>, Uuid)> {
    let rest = raw
        .strip_prefix("t:")
        .ok_or_else(|| AppError::BadRequest("分页游标无效".into()))?;
    let (ts, id) = rest
        .rsplit_once(',')
        .ok_or_else(|| AppError::BadRequest("分页游标无效".into()))?;
    let ts = DateTime::parse_from_rfc3339(ts)
        .map_err(|_| AppError::BadRequest("分页游标无效".into()))?
        .with_timezone(&Utc);
    let id = Uuid::parse_str(id).map_err(|_| AppError::BadRequest("分页游标无效".into()))?;
    Ok((ts, id))
}

fn encode_hot_cursor(likes: i32, ts: DateTime<Utc>, id: Uuid) -> String {
    format!("h:{likes},{},{id}", ts.to_rfc3339())
}

fn decode_hot_cursor(raw: &str) -> AppResult<(i32, DateTime<Utc>, Uuid)> {
    let rest = raw
        .strip_prefix("h:")
        .ok_or_else(|| AppError::BadRequest("分页游标无效".into()))?;
    let (head, id) = rest
        .rsplit_once(',')
        .ok_or_else(|| AppError::BadRequest("分页游标无效".into()))?;
    let (likes, ts) = head
        .rsplit_once(',')
        .ok_or_else(|| AppError::BadRequest("分页游标无效".into()))?;
    let likes: i32 = likes
        .parse()
        .map_err(|_| AppError::BadRequest("分页游标无效".into()))?;
    let ts = DateTime::parse_from_rfc3339(ts)
        .map_err(|_| AppError::BadRequest("分页游标无效".into()))?
        .with_timezone(&Utc);
    let id = Uuid::parse_str(id).map_err(|_| AppError::BadRequest("分页游标无效".into()))?;
    Ok((likes, ts, id))
}

#[derive(sqlx::FromRow)]
struct WorkRow {
    id: Uuid,
    user_id: Uuid,
    title: String,
    caption: String,
    category: String,
    board_size: i32,
    color_count: i32,
    peg_count: i32,
    preview_key: String,
    like_count: i32,
    favorite_count: i32,
    comment_count: i32,
    created_at: DateTime<Utc>,
    nickname: String,
    avatar_url: Option<String>,
    liked: bool,
    favorited: bool,
}

#[derive(sqlx::FromRow)]
struct CommentRow {
    id: Uuid,
    content: String,
    created_at: DateTime<Utc>,
    author_id: Uuid,
    nickname: String,
    avatar_url: Option<String>,
}

#[derive(sqlx::FromRow)]
struct DraftPubRow {
    #[allow(dead_code)]
    id: Uuid,
    user_id: Uuid,
    title: String,
    board_size: i32,
    color_count: i32,
    peg_count: i32,
    preview_key: String,
    #[allow(dead_code)]
    is_official: bool,
}

#[derive(sqlx::FromRow)]
struct DraftOwner {
    id: Uuid,
    user_id: Uuid,
}
