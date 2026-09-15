//! 自己反省: 決済されたトレードを LLM に振り返らせ、教訓「候補」を作る。
//!
//! 1回の負けから絶対ルールを作らない。候補は無効状態で保存し、
//! 同種（同カテゴリ・同ペア）の候補が閾値回数に達したときだけ採用する。

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::ctrader::CandleBar;
use crate::llm::LlmClient;
use crate::server::types::{CloseReason, CoTLog, LessonLearned, TradeHistory};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reflection {
    /// 根本原因（1〜2文）
    pub root_cause: String,
    /// "RISK" | "TIMING" | "PATTERN" | "NEWS"
    pub category: String,
    /// 読解上の注意として書いた1文の教訓
    pub lesson: String,
    /// 判断自体は妥当で、結果だけが悪かった場合 true（教訓にしない）
    pub decision_was_sound: bool,
}

pub fn reflection_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["root_cause", "category", "lesson", "decision_was_sound"],
        "properties": {
            "root_cause": { "type": "string" },
            "category": { "type": "string", "enum": ["RISK", "TIMING", "PATTERN", "NEWS"] },
            "lesson": { "type": "string" },
            "decision_was_sound": { "type": "boolean" }
        }
    })
}

pub fn build_reflection_prompt(trade: &TradeHistory, cot: Option<&CoTLog>, post_bars: &[CandleBar]) -> String {
    let bars_text: String = post_bars
        .iter()
        .map(|b| format!("{} o={:.5} h={:.5} l={:.5} c={:.5}", b.timestamp.format("%m-%d %H:%M"), b.open, b.high, b.low, b.close))
        .collect::<Vec<_>>()
        .join("\n");
    let cot_text = cot
        .map(|c| {
            format!(
                "環境認識: {}\n注文の攻防: {}\n無効化ライン: {}\n反対材料: {}\n判断理由: {}\n確信度: {:.2}",
                c.macro_context, c.order_flow, c.invalidation, c.conflicts, c.reasoning, c.confidence
            )
        })
        .unwrap_or_else(|| "（エントリー時の判断ログなし）".to_string());
    let reason = match trade.close_reason {
        CloseReason::StopLoss => "損切り（SL到達）",
        CloseReason::TakeProfit => "利確（TP到達）",
        CloseReason::Manual => "手動決済",
        CloseReason::CircuitBreaker => "サーキットブレーカー",
    };

    format!(
        r#"あなたは客観的で厳格なプロFXトレーダー兼リスクアナリストです。
以下のトレードを振り返り、根本原因と「今後同じ局面で特に注意して読むための1文」を返してください。

【トレード】
- 通貨ペア: {pair}
- 方向: {side}
- エントリー: {entry:.5} / SL: {sl:.5} / TP: {tp:.5}
- 決済: {close:.5}（{reason}）
- 損益: {pnl:.1} pips
- 保有: {open} 〜 {closed}

【エントリー時の判断】
{cot}

【エントリー後の5M足（時系列）】
{bars}

【指示】
- 「なぜ負けたか」ではなく「エントリー時点で見えていたはずなのに読み落とした事実は何か」を特定してください。
- 判断は妥当で結果だけが悪かった（読み落としがない）場合は decision_was_sound を true にし、lesson は空文字にしてください。
- lesson は「〇〇の局面では△△に注意して読む」という読解上の注意として書き、「必ずHOLD」のような絶対ルールにはしないでください。
- category は RISK（SL幅・RR・ロット）/ TIMING（時間帯・セッション）/ PATTERN（形の読み違い）/ NEWS（指標・イベント）から選んでください。
"#,
        pair = trade.symbol,
        side = trade.side,
        entry = trade.entry_price,
        sl = trade.stop_loss,
        tp = trade.take_profit,
        close = trade.close_price,
        reason = reason,
        pnl = trade.pnl_pips,
        open = trade.open_time,
        closed = trade.close_time,
        cot = cot_text,
        bars = bars_text,
    )
}

/// LLM に反省させる
pub async fn reflect(llm: &LlmClient, trade: &TradeHistory, cot: Option<&CoTLog>, post_bars: &[CandleBar]) -> Result<Reflection> {
    let prompt = build_reflection_prompt(trade, cot, post_bars);
    let v = llm.infer_json(&prompt, reflection_schema()).await?;
    Ok(serde_json::from_value(v)?)
}

/// 採用ルール: 同カテゴリ・同ペアの候補（無効）が `threshold` 件以上になったら最新の候補を有効化する。
/// 有効化した候補の ID を返す。
pub fn maybe_adopt(lessons: &mut [LessonLearned], symbol: &str, category: &str, threshold: usize, max_active: usize) -> Option<String> {
    let active_count = lessons.iter().filter(|l| l.active).count();
    if active_count >= max_active {
        return None;
    }
    let mut candidates: Vec<&mut LessonLearned> = lessons
        .iter_mut()
        .filter(|l| !l.active && l.category == category && (l.symbol == symbol || l.symbol == "ALL"))
        .collect();
    if candidates.len() < threshold {
        return None;
    }
    // created_at は "YYYY-MM-DD HH:MM:SS" なので文字列比較で最新が最大
    candidates.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let newest = candidates.first_mut()?;
    newest.active = true;
    Some(newest.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lesson(id: &str, cat: &str, created: &str, active: bool) -> LessonLearned {
        LessonLearned {
            id: id.into(),
            created_at: created.into(),
            symbol: "USDJPY".into(),
            rule: "r".into(),
            context: "c".into(),
            active,
            trigger_trade_id: None,
            category: cat.into(),
        }
    }

    #[test]
    fn adopts_only_after_threshold_and_respects_cap() {
        let mut ls = vec![
            lesson("a", "TIMING", "2026-09-01 00:00:00", false),
            lesson("b", "TIMING", "2026-09-02 00:00:00", false),
        ];
        assert!(maybe_adopt(&mut ls, "USDJPY", "TIMING", 3, 10).is_none());
        ls.push(lesson("c", "TIMING", "2026-09-03 00:00:00", false));
        assert_eq!(maybe_adopt(&mut ls, "USDJPY", "TIMING", 3, 10).as_deref(), Some("c"));
        assert!(ls.iter().find(|l| l.id == "c").unwrap().active);

        let mut full: Vec<LessonLearned> = (0..10).map(|i| lesson(&format!("x{i}"), "RISK", "2026-09-01 00:00:00", true)).collect();
        full.extend((0..3).map(|i| lesson(&format!("y{i}"), "RISK", "2026-09-02 00:00:00", false)));
        assert!(maybe_adopt(&mut full, "USDJPY", "RISK", 3, 10).is_none());
    }

    #[test]
    fn prompt_contains_trade_and_bars() {
        let t = TradeHistory {
            id: "t".into(),
            symbol: "USDJPY".into(),
            side: "BUY".into(),
            volume_lots: 0.1,
            entry_price: 154.2,
            close_price: 154.1,
            stop_loss: 154.1,
            take_profit: 154.5,
            pnl_pips: -10.0,
            pnl_amount: -1000.0,
            close_reason: CloseReason::StopLoss,
            open_time: "a".into(),
            close_time: "b".into(),
            cot_log_id: None,
        };
        let p = build_reflection_prompt(&t, None, &[]);
        assert!(p.contains("損切り"));
        assert!(p.contains("USDJPY"));
        assert!(reflection_schema()["required"].as_array().unwrap().len() == 4);
    }
}
