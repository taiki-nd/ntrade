use crate::strategy::types::PriceActionInput;

/// プロンプトビルダー
pub struct PromptBuilder {
    lessons: Vec<String>,
}

impl PromptBuilder {
    pub fn new() -> Self {
        Self {
            lessons: Vec::new(),
        }
    }

    pub fn with_lessons(mut self, lessons: Vec<String>) -> Self {
        self.lessons = lessons;
        self
    }

    /// プライスアクションデータからLLMへの完全プロンプトを構築
    pub fn build_inference_prompt(&self, input: &PriceActionInput) -> String {
        let input_json = serde_json::to_string_pretty(input).unwrap_or_default();

        let lessons_section = if self.lessons.is_empty() {
            String::new()
        } else {
            let mut s = String::from("\n【過去の失敗トレードから得られた厳守ルール（教訓リスト）】\n");
            for (idx, lesson) in self.lessons.iter().enumerate() {
                s.push_str(&format!("{}. {}\n", idx + 1, lesson));
            }
            s.push_str("※ 上記の教訓に抵触する場合は、確信度が高く見えても必ず「HOLD（見送り）」とすること。\n");
            s
        };

        format!(
            r#"あなたはプライスアクションとマルチタイムフレーム分析を専門とするプロFXトレーダーです。
遅行指標の過度な盲信を避け、「ローソク足の幾何学的形状」「サポレジラインでの拒絶（Rejection）」「市場の流動性ハント（騙しブレイク）」を最重要視して売買判断を下してください。

【厳格なエントリー条件】
1. 4H・1Hの上位足トレンドに逆行するエントリーは原則禁止（上位足順張りを徹底）。
2. 5M足で明確な3大トリガー（①レジェクション・ピンバー、②包み足/インガルフィング、③フェイクアウト/流動性ハント）が発生していない場合は、必ず「HOLD（見送り）」とすること。
3. ストップロス（SL）は必ず「プライスアクションの根拠が崩れる客観的ライン（ピンバーのヒゲ先端外側など）」に設定すること。
4. 想定リスクリワード比（SL幅に対するTP幅の比率）が最低 1 : 1.5 未満の場合は見送ること。
5. 確信度が 0.75 未満の場合は、無理にトレードせず「HOLD（見送り）」とすること。
{}
【現在の相場データ (JSON)】
```json
{}
```

【タスク】
上記の相場データおよびプライスアクション特徴量を精査し、以下のJSONフォーマットのみを出力してください。
Markdownコードブロック（```json ... ```）で囲んで出力してください。余計な前置きや後置きの解説テキストは一切不要です。

【出力フォーマット】
```json
{{
  "action": "BUY | SELL | HOLD",
  "confidence": 0.88,
  "entry_type": "MARKET",
  "entry_price": 154.205,
  "stop_loss": 154.080,
  "take_profit": 154.550,
  "risk_reward_ratio": 2.76,
  "price_action_analysis": {{
    "macro_bias": "上位足(4H/1H)のトレンドおよび環境認識",
    "trigger_pattern": "5M足で検出された具体的プライスアクショントリガーとその根拠",
    "invalidation_point": "シナリオ無効化価格（SL位置の客観的理由）"
  }},
  "reasoning": "この判断に至った論理的根拠（CoT）、リスクリワード評価、スプレッド考慮"
}}
```
"#,
            lessons_section, input_json
        )
    }
}
