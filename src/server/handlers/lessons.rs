use axum::{
    extract::{Path, State},
    response::Json,
};
use chrono::Utc;
use tracing::info;

use crate::server::state::AppState;
use crate::server::types::{ApiResponse, CreateLessonRequest, LessonLearned};

/// GET /api/lessons
/// 自己反省ルール（教訓）一覧を返却
pub async fn get_lessons(State(state): State<AppState>) -> Json<ApiResponse<Vec<LessonLearned>>> {
    match state.with_db(|db| db.lessons()).await {
        Ok(lessons) => Json(ApiResponse::ok(lessons)),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// POST /api/lessons
/// 新規の自己反省ルール（教訓）を登録
pub async fn create_lesson(
    State(state): State<AppState>,
    Json(payload): Json<CreateLessonRequest>,
) -> Json<ApiResponse<LessonLearned>> {
    info!("Creating new lesson rule: {}", payload.rule);

    let new_lesson = LessonLearned {
        id: format!("les-{}", Utc::now().timestamp_millis()),
        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        symbol: payload.symbol,
        rule: payload.rule,
        context: payload.context,
        active: payload.active,
        trigger_trade_id: None,
        category: payload.category,
    };

    let saved = new_lesson.clone();
    match state.with_db(move |db| db.upsert_lesson(&saved)).await {
        Ok(()) => Json(ApiResponse::ok_msg(new_lesson, "教訓ルールを登録しました")),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// POST /api/lessons/:id/toggle
/// 教訓ルールの有効/無効状態を切り替え
pub async fn toggle_lesson(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<LessonLearned>> {
    info!("Toggling lesson rule id: {}", id);

    let key = id.clone();
    let res = state
        .with_db(move |db| {
            let Some(mut target) = db.lesson(&key)? else { return Ok(None) };
            target.active = !target.active;
            db.upsert_lesson(&target)?;
            Ok(Some(target))
        })
        .await;
    match res {
        Ok(Some(updated)) => Json(ApiResponse::ok_msg(updated, "教訓ルールの適用状態を切り替えました")),
        Ok(None) => Json(ApiResponse::err(format!("Lesson {} not found", id))),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// DELETE /api/lessons/:id
/// 教訓ルールを削除
pub async fn delete_lesson(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    info!("Deleting lesson rule id: {}", id);

    let key = id.clone();
    match state.with_db(move |db| db.delete_lesson(&key)).await {
        Ok(true) => Json(ApiResponse::ok_msg((), "教訓ルールを削除しました")),
        Ok(false) => Json(ApiResponse::err(format!("Lesson {} not found", id))),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}
