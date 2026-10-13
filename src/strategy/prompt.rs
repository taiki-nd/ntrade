use crate::fundamentals::FundamentalFacts;
use crate::snapshot::MarketSnapshot;

/// プロンプトビルダー
///
/// 方向（地合い）はファンダメンタルズの事実から LLM が判断する。チャート画像やローソク足の形の読み
/// （プライスアクション）は使わない。価格の数値（現在値・ATR・前日高安・スイングなど）は、
/// エントリー・SL・TP をどこに置くかを決めるためだけに渡す。
///
/// 数値ルール（RR下限、確信度閾値、スプレッド上限など）は意図的に書かない。
/// それらはプログラム側の事後ガードが扱う。ここでは「読む順序」と「姿勢」だけを指示する。
pub struct PromptBuilder {
    lessons: Vec<String>,
    fundamentals: Option<FundamentalFacts>,
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptBuilder {
    pub fn new() -> Self {
        Self { lessons: Vec::new(), fundamentals: None }
    }

    pub fn with_lessons(mut self, lessons: Vec<String>) -> Self {
        self.lessons = lessons;
        self
    }

    /// ファンダメンタルズの事実。渡さない（リプレイなど）場合は「取得していない」と明示する
    pub fn with_fundamentals(mut self, facts: FundamentalFacts) -> Self {
        self.fundamentals = Some(facts);
        self
    }

    /// 推論プロンプトを構築する
    pub fn build(&self, snapshot: &MarketSnapshot) -> String {
        let lessons_section = if self.lessons.is_empty() {
            String::new()
        } else {
            let mut s = String::from("\n【過去の失敗から得た注意点】\n");
            for (i, l) in self.lessons.iter().enumerate() {
                s.push_str(&format!("{}. {}\n", i + 1, l));
            }
            s.push_str("※ これらは判断を狭める禁止則ではなく、該当局面で特に注意して読むための観点です。\n");
            s
        };

        let fundamentals_section = match &self.fundamentals {
            Some(f) if !f.is_empty() => format!(
                r#"【ファンダメンタルズの事実 (JSON)】
公的統計・中央銀行・米財務省/日本財務省・CFTC 建玉明細・経済指標の予定・公式発表のタイトル・関連銘柄の日足騰落です。
どれも事実だけで、「上がりそう/下がりそう」といった評価は付けていません。どの事実がこの銘柄にどちら向きに効くかは、あなたが判断してください。
- series: 各系列の最新値（latest）・前回値（previous）と、約1か月・3か月・12か月前の値。date は観測日です。月次の統計は発表が遅れるので、日付を見て鮮度を確かめてください。
- positioning_cftc: 投機筋（managed_money / noncommercial）のロング・ショート・ネット（火曜時点、金曜公表）と、52週のネットの高値・安値。
- economic_calendar: 指標の予定時刻（UTC）。released=true は予定時刻を過ぎたもの（このデータに結果の数値は無い）。予定が近い指標は、入るか・待つか・SL をどこに置くかを決める材料になります。
- official_releases: 中央銀行・統計機関の公式発表のタイトルと時刻（本文は含みません）。
- related_markets: 関連銘柄の日足終値と騰落率（%）。
- unavailable: 取得できなかった情報源。欠けている事実を推測で補わないでください。
```json
{json}
```
"#,
                json = f.to_json_pretty()
            ),
            _ => "【ファンダメンタルズの事実】\n今回は取得できていません。方向の根拠が無いので、原則として HOLD とし、そのことを analysis.fundamental_view に書いてください。\n".to_string(),
        };

        let snapshot_json = snapshot.to_json_pretty();

        format!(
            r#"あなたは FX・貴金属・株価指数・エネルギーの CFD をレバレッジを掛けて取引する、ファンダメンタルズ分析を専門とするプロの裁量トレーダーです。
「ファンダメンタルズの事実 (JSON)」から相場の地合い（どちらに力が掛かりやすいか）を判断し、「価格の数値 (JSON)」を使ってエントリー・損切り・利確の価格を決めて、売買判断を返してください。

【使ってよい情報】
- 判断の材料は、このプロンプトに含まれる事実だけです。Web 検索や外部の情報は使えません。
- あなたが学習で得た知識は、事実の意味を解釈するための一般知識（例: 実質金利と金価格の関係、金利差と為替の関係）としてだけ使ってください。
  「最近の相場や出来事についての記憶」で事実を上書きしたり、データに無い出来事を前提にしたりしてはいけません。あなたの記憶はデータの日付より古い可能性があります。
- 方向はファンダメンタルズの事実から決めます。ローソク足の形やチャートパターンから方向を決めないでください。
- 価格の数値は、今の価格が直近の値幅のどこにあるか、損切りをどこに置けば想定が外れたと分かるかを決めるために使います。

{fundamentals_section}
【読む順序】
1. ファンダメンタルズの地合い
   - この銘柄の価格を動かす主な要因（金利・金利差・実質金利・ドルの強さ・物価と雇用・リスク選好・需給・投機筋の偏り）について、事実はどちらに傾いているか。最近どう変化したか。
   - 事実が乏しい・古い・互いに矛盾する場合は、そう判断してください。無理に方向を作らないこと。
2. イベントリスク
   - 予定されている重要指標や、直近の公式発表はあるか。それは今エントリーするリスクをどう変えるか（今入る・発表後まで待つ・SL を広く取る など）。
   - `instrument.next_market_change` が近い場合（週末・休場前）、持ち越しのリスクも考える。
3. 価格の位置と損切り
   - 現在値（current_price）が、前日高安・当日高安・スイング高安・ATR（volatility）から見てどの位置にあるか。
   - 地合いの判断が間違っていたと分かる価格はどこか。それが損切りの位置です。ATR に対して狭すぎると日々の値動きだけで刈られます。
   - 利確は、地合いが続いた場合に現実的に届く価格に置く。
   - 今は入らないが、価格がどこまで来たら入るか（あるいは指標発表後に）という待ち条件があれば、条件付きプランとして具体的な価格と条件を書いてください。
4. 反対材料
   - 自分の判断に反する事実を必ず1つ以上挙げてください（逆向きの事実・指標発表・投機筋の偏りの行き過ぎ・ボラティリティなど）。

【判断の姿勢】
- レバレッジを掛けた取引です。ロットはシステムが SL 幅と許容リスクから決めるので、あなたは「どこで間違いと分かるか」に集中して SL を置いてください。
- 数値は JSON の価格を使って正確に書く。価格の桁数は `instrument.digits` に合わせる。
- 自信が無い場合は HOLD とし、待ち条件を提示する。HOLD は失敗ではない。
- `recent_decisions` に前回までの判断がある場合、前回の地合いの判断が継続しているか崩れたかを明示する。事実が変わっていないのに判断を反転させない。
- `account_state.triggered_plan` がある場合、前回までに立てた条件付きプランの成立条件が、直近の確定足で満たされています。プランは自動では発注されません。今の事実と価格で、このプランどおりに入るかを改めて判断してください。入るなら今の価格からエントリーすることになるので、SL/TP とリスクリワードは今の価格から測り直してください。
{lessons_section}
【価格の数値 (JSON)】
```json
{snapshot_json}
```

【出力】
指定された JSON Schema に従って結果を返してください。各フィールドの意味:
- action: BUY / SELL / HOLD
- confidence: 0.0〜1.0（0.1刻み）。地合いの判断とエントリー機会に対する確信度。0.62のような細かい小数ではなく必ず0.1刻みで指定する:
  - 0.0〜0.4: 事実が拮抗・乏しい・古い（HOLD）
  - 0.5: どちらかに傾いているが、根拠が薄い / イベント待ち（HOLD）
  - 0.6: 地合いは読めるが、価格やタイミングの条件が未達（条件付きプラン待ちのHOLDなど）
  - 0.7: 地合いの根拠が複数の事実で裏付けられ、リスクリワードも見合っている（エントリー検討水準）
  - 0.8以上: 金利・物価・ポジション・関連市場などの事実がそろって同じ方向を示し、目立った反対材料やイベントリスクが無い
- entry_type: MARKET（成行で今入る）/ LIMIT / CONDITIONAL（今は入らず conditional_plan を待つ）/ null
- entry_price, stop_loss, take_profit: BUY/SELL の場合は必須。stop_loss は analysis.invalidation の価格と一致させる
- analysis.fundamental_view: ファンダメンタルズの読み。どの事実（系列名・日付・値）を根拠に、どちらの地合いと読んだか。予定指標などのイベントリスク。事実が足りない場合はそう書く
- analysis.price_levels: 価格の位置（現在値が前日高安・スイング・ATR から見てどこにあるか）と、エントリー・利確の価格の根拠
- analysis.invalidation: 損切りの価格と、その価格で地合いの判断が間違いと分かる理由
- analysis.conflicts: 反対材料
- conditional_plan: 待ち条件がある場合のみ。wait_for / then_action / invalidate_if / expires_at（"YYYY-MM-DD HH:MM:SS UTC"）に加え、プログラムが機械的に判定するための構造化条件を必ず埋める:
  - trigger_price + trigger_condition（CLOSE_ABOVE / CLOSE_BELOW）: 5M確定足の終値がこの価格をこの向きに抜けたら成立
  - invalidate_price + invalidate_condition: 5M確定足の終値がこの価格をこの向きに抜けたら破棄
  - stop_loss / take_profit: 成立時に使う値
  条件を1つの価格で表せない場合は、最も本質的な1条件に絞って書く
- reasoning: 判断に至った論理の要約
"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{AccountState, SnapshotConfig, SnapshotInput};

    fn snapshot() -> MarketSnapshot {
        MarketSnapshot::build(
            &SnapshotInput {
                pair: "XAUUSD",
                bars_4h: &[],
                bars_1h: &[],
                bars_15m: &[],
                bars_5m: &[],
                spread_pips: None,
                current_price: Some(2650.0),
                now: None,
                account_state: AccountState::default(),
            },
            &SnapshotConfig::default(),
        )
    }

    #[test]
    fn prompt_carries_fundamental_facts_and_forbids_outside_information() {
        let facts = FundamentalFacts {
            symbol: "XAUUSD".into(),
            as_of: "2026-10-04 00:00 UTC".into(),
            series: vec![crate::fundamentals::SeriesFact {
                id: "ust_real:10Y".into(),
                title: "U.S. Treasury real yield (TIPS) 10Y".into(),
                units: None,
                frequency: None,
                source: "UST".into(),
                latest: Some(crate::fundamentals::sources::Observation { date: "2026-10-02".into(), value: 2.92 }),
                previous: None,
                about_1m_ago: None,
                about_3m_ago: None,
                about_12m_ago: None,
            }],
            ..Default::default()
        };
        let p = PromptBuilder::new().with_fundamentals(facts).build(&snapshot());
        assert!(p.contains("ust_real:10Y") && p.contains("2.92"));
        assert!(p.contains("Web 検索や外部の情報は使えません"));
        assert!(p.contains("analysis.fundamental_view"));
        // チャート画像は渡さない（プライスアクションで方向を決めない）
        assert!(!p.contains(".png") && !p.contains("Readツール"));

        // 事実が無いときはその旨を明示する
        let p = PromptBuilder::new().build(&snapshot());
        assert!(p.contains("今回は取得できていません"));
    }
}
