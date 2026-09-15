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
    let lock = state.lessons.read().await;
    Json(ApiResponse::ok(lock.clone()))
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

    let mut lock = state.lessons.write().await;
    lock.insert(0, new_lesson.clone());

    Json(ApiResponse::ok_msg(new_lesson, "教訓ルールを登録しました"))
}

/// POST /api/lessons/:id/toggle
/// 教訓ルールの有効/無効状態を切り替え
pub async fn toggle_lesson(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<LessonLearned>> {
    info!("Toggling lesson rule id: {}", id);

    let mut lock = state.lessons.write().await;
    if let Some(target) = lock.iter_mut().find(|l| l.id == id) {
        target.active = !target.active;
        let updated = target.clone();
        Json(ApiResponse::ok_msg(updated, "教訓ルールの適用状態を切り替えました"))
    } else {
        Json(ApiResponse {
            success: false,
            data: None,
            message: Some(format!("Lesson {} not found", id)),
        })
    }
}

/// DELETE /api/lessons/:id
/// 教訓ルールを削除
pub async fn delete_lesson(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    info!("Deleting lesson rule id: {}", id);

    let mut lock = state.lessons.write().await;
    let initial_len = lock.len();
    lock.retain(|l| l.id != id);

    if lock.len() < initial_len {
        Json(ApiResponse::ok_msg((), "教訓ルールを削除しました"))
    } else {
        Json(ApiResponse {
            success: false,
            data: None,
            message: Some(format!("Lesson {} not found", id)),
        })
    }
}
