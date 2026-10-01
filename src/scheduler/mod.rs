//! 常駐スケジューラ: 5分足確定ごとに全ペアの判断サイクルを並行に回し、ポジション同期と自己反省を行う。
//!
//! 対象ペアと足確定後の待ち秒数は取引設定（`settings::TradingSettings`、画面から変更可）を
//! 毎サイクル読み直すので、変更は再起動なしで次の足から反映される。
//!
//! 環境変数:
//! - `NTRADE_SCHEDULER=off`      … ループを起動しない

use chrono::{DateTime, Duration, TimeZone, Utc};
use tracing::{info, warn};

use crate::ctrader::{BarPeriod, CandleBar};
use crate::reflection;
use crate::server::handlers::decide::run_cycle_locked;
use crate::server::state::AppState;
use crate::server::types::{BotState, CloseReason, LessonLearned, TradeHistory};

pub const LESSON_ADOPT_THRESHOLD: usize = 3;
pub const LESSON_MAX_ACTIVE: usize = 10;

/// 次の 5M 境界 + 遅延
pub fn next_tick(now: DateTime<Utc>, delay_secs: i64) -> DateTime<Utc> {
    let step = 300;
    let next = (now.timestamp() / step + 1) * step;
    Utc.timestamp_opt(next, 0).single().unwrap_or(now) + Duration::seconds(delay_secs)
}

pub async fn run(state: AppState) {
    {
        let s = state.settings.read().await;
        info!(pairs = ?s.pairs, delay = s.bar_delay_secs, "scheduler started (5M cadence)");
    }

    loop {
        let delay = state.settings.read().await.bar_delay_secs;
        let now = Utc::now();
        let at = next_tick(now, delay);
        let wait = (at - now).to_std().unwrap_or_default();
        tokio::time::sleep(wait).await;

        let bot_state = *state.bot_state.read().await;
        if bot_state != BotState::Running {
            info!(?bot_state, "scheduler: bot not running, skipping cycle");
            continue;
        }
        let connected = state.ctrader_service.read().await.is_some();
        if !connected {
            warn!("scheduler: cTrader not connected, skipping cycle (no live bars)");
            continue;
        }

        // ペアごとに並行に回す。発注まわりの排他は run_cycle_locked / order_lock 側で取る
        let pairs = state.pairs().await;
        let mut cycles = tokio::task::JoinSet::new();
        for pair in pairs {
            let state = state.clone();
            cycles.spawn(async move {
                match run_cycle_locked(&state, &pair).await {
                    Ok(r) => info!(pair, action = ?r.decision.action, guard = r.guard.passed, executed = r.executed, "cycle ok"),
                    Err(e) => warn!(pair, "cycle failed: {e:#}"),
                }
            });
        }
        while let Some(res) = cycles.join_next().await {
            if let Err(e) = res {
                warn!("cycle task panicked: {e}");
            }
        }

        if let Err(e) = after_cycle(&state).await {
            warn!("post-cycle sync failed: {e:#}");
        }
    }
}

/// サイクル後: ブローカー同期 → 決済トレードの自己反省
///
/// 照合そのものは `AppState::reconcile_positions`（常駐ループと共有）に任せる。
pub async fn after_cycle(state: &AppState) -> anyhow::Result<()> {
    let closed = state.reconcile_positions().await?;
    reflect_on_closed(state, closed).await;
    Ok(())
}

/// 確定した決済の自己反省（損切りのみ。判断ログとエントリー後の足を渡す）。
/// 決済は判断サイクルと照合ループのどちらからでも確定しうるので、両方からここを通す。
pub async fn reflect_on_closed(state: &AppState, closed: Vec<TradeHistory>) {
    for t in closed.into_iter().filter(|t| matches!(t.close_reason, CloseReason::StopLoss | CloseReason::Invalidated)) {
        let state = state.clone();
        let post_bars: Vec<CandleBar> = {
            let db = state.ctrader_service.read().await.clone();
            match db {
                Some(c) => c.get_trendbars(&t.symbol, BarPeriod::M5, 24).await.unwrap_or_default(),
                None => Vec::new(),
            }
        };
        tokio::spawn(async move {
            // 反省の材料は LLM の判断そのもの。プラン成立の再判断はそれ自体が LLM 判断なのでそのまま使い、
            // 旧データの機械的な成立ログ（cot-plan-*）だけ元の判断まで辿る
            let cot = match t.cot_log_id.clone() {
                Some(id) => state
                    .with_db(move |db| {
                        let Some(log) = db.cot_log(&id)? else { return Ok(None) };
                        match log.origin_cot_log_id.as_deref() {
                            Some(origin) if log.id.starts_with("cot-plan-") => Ok(db.cot_log(origin)?.or(Some(log))),
                            _ => Ok(Some(log)),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| {
                        warn!(trade = %t.id, "failed to load CoT log for reflection: {e:#}");
                        None
                    }),
                None => None,
            };
            match reflection::reflect(&state.llm, &t, cot.as_ref(), &post_bars).await {
                Ok(r) if !r.decision_was_sound && !r.lesson.trim().is_empty() => {
                    let lesson = LessonLearned {
                        id: format!("les-{}", Utc::now().timestamp_millis()),
                        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        symbol: t.symbol.clone(),
                        rule: r.lesson.clone(),
                        context: r.root_cause.clone(),
                        active: false,
                        trigger_trade_id: Some(t.id.clone()),
                        category: r.category.clone(),
                    };
                    let (symbol, category) = (t.symbol.clone(), r.category.clone());
                    let adopted = state
                        .with_db(move |db| {
                            db.upsert_lesson(&lesson)?;
                            let mut lessons = db.lessons()?;
                            let adopted = reflection::maybe_adopt(&mut lessons, &symbol, &category, LESSON_ADOPT_THRESHOLD, LESSON_MAX_ACTIVE);
                            if let Some(l) = adopted.as_ref().and_then(|id| lessons.iter().find(|l| &l.id == id)) {
                                db.upsert_lesson(l)?;
                            }
                            Ok(adopted)
                        })
                        .await;
                    match adopted {
                        Ok(Some(id)) => info!(lesson = %id, category = %r.category, "lesson adopted (threshold reached)"),
                        Ok(None) => info!(category = %r.category, "lesson candidate stored (inactive)"),
                        Err(e) => warn!(trade = %t.id, "failed to store lesson: {e:#}"),
                    }
                }
                Ok(r) => info!(trade = %t.id, sound = r.decision_was_sound, "reflection: no lesson"),
                Err(e) => warn!(trade = %t.id, "reflection failed: {e:#}"),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_tick_aligns_to_five_minutes() {
        let now = Utc.with_ymd_and_hms(2026, 9, 15, 9, 3, 20).unwrap();
        assert_eq!(next_tick(now, 15), Utc.with_ymd_and_hms(2026, 9, 15, 9, 5, 15).unwrap());
        let on_boundary = Utc.with_ymd_and_hms(2026, 9, 15, 9, 5, 0).unwrap();
        assert_eq!(next_tick(on_boundary, 0), Utc.with_ymd_and_hms(2026, 9, 15, 9, 10, 0).unwrap());
    }
}
