use std::path::Path;

use crate::snapshot::{ChartSet, MarketSnapshot};

/// プロンプトビルダー
///
/// 数値ルール（RR下限、確信度閾値、スプレッド上限など）は意図的に書かない。
/// それらはプログラム側の事後ガードが扱う。ここでは「読む順序」と「姿勢」だけを指示する。
pub struct PromptBuilder {
    lessons: Vec<String>,
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptBuilder {
    pub fn new() -> Self {
        Self { lessons: Vec::new() }
    }

    pub fn with_lessons(mut self, lessons: Vec<String>) -> Self {
        self.lessons = lessons;
        self
    }

    /// Snapshot と画像パスから推論プロンプトを構築する
    pub fn build(&self, snapshot: &MarketSnapshot, charts: &ChartSet) -> String {
        let images = charts
            .ordered()
            .iter()
            .map(|(tf, p)| format!("- {}: {}", tf, absolute(p)))
            .collect::<Vec<_>>()
            .join("\n");

        let lessons_section = if self.lessons.is_empty() {
            String::new()
        } else {
            let mut s = String::from("\n【過去の失敗から得た読解上の注意】\n");
            for (i, l) in self.lessons.iter().enumerate() {
                s.push_str(&format!("{}. {}\n", i + 1, l));
            }
            s.push_str("※ これらは判断を狭める禁止則ではなく、該当局面で特に注意して読むための観点です。\n");
            s
        };

        let snapshot_json = snapshot.to_json_pretty();

        format!(
            r#"あなたはプライスアクションとマルチタイムフレーム分析を専門とするプロFXトレーダーです。
これから渡す「4枚のチャート画像」と「客観的事実のJSON」を読み、相場の攻防を読み解いて売買判断を返してください。

【最初に必ず行うこと】
以下の4枚の画像を、Readツールで**すべて**読んでください。読み終わるまで分析を始めてはいけません。
{images}
画像は時間足ごとに1枚です。ローソク足、EMA20（オレンジ）、EMA50（青）、価格ラベル付きの水平線（PDH/PDL=前日高安、today H/L=当日高安、asia H/L=アジア時間高安、round=キリ番、swing H/L=スイング高安）、現在価格（黄）が描かれています。
線は「そこに価格の事実がある」ことを示すだけで、支持か抵抗かは描いていません。意味づけはあなたが行ってください。

【読む順序】
1. 環境認識（4H / 1H の画像）
   - 現在どちらの勢力が優勢か。押し目・戻りを作っている最中か、天井圏・底値圏か、方向感の無いレンジか。
   - 直近で買い手と売り手が明確に戦った価格帯はどこか。今の価格はそこからどの位置にいるか。
2. 注文の攻防（15M / 5M の画像と、JSONの生OHLC）
   - 直近数本のローソク足から、どちらが主導権を握り直したか。
   - ブレイクを試みて拒絶された兆候、あるいは受け入れられた兆候はあるか。
   - 上位足の文脈とこの攻防は整合しているか、矛盾しているか。
3. シナリオと無効化ライン
   - エントリーするなら、どこを実体で抜けたらこの相場観は完全に間違いになるか。それが損切りの位置です。
   - 今は入らないが、何が起きたら入るか。条件付きプランとして具体的な価格と条件を書いてください。
4. 反対材料
   - 自分のシナリオに反する材料を必ず1つ以上挙げてください。ボラティリティ、時間帯、上位足との矛盾など。

【参考知識: 攻防の痕跡の読み方】
- 長いヒゲ: その方向への試みが拒絶された痕跡。ただし、どの価格帯で出たか、次の足で追認されたかを見なければ意味は確定しない。
- 包み足: 直前の足の値幅を丸ごと否定した勢い。押し目・戻りの終わりで出るか、伸び切った所で出るかで意味が変わる。
- 高値/安値の一時的なブレイクからの即時反転: ブレイクを期待した注文とその損切りを巻き込んだ流動性の動き。
これらは「形が出たら入る」ためのルールではありません。なぜそこで買い手/売り手が動いたかを説明できない場合は見送ってください。

【判断の姿勢】
- 上位足に逆行する場合は、逆行する明確な根拠を言語化できる場合のみ許容する。
- 数値は画像ではなくJSONの生OHLC・水平線の価格を使って正確に書く。
- 自信が無い場合は HOLD とし、待ち条件を提示する。HOLD は失敗ではない。
- `recent_decisions` に前回までの判断がある場合、前回立てたシナリオが継続しているか崩れたかを明示する。理由なく判断を反転させない。
{lessons_section}
【客観的事実 (JSON)】
```json
{snapshot_json}
```

【出力】
指定された JSON Schema に従って結果を返してください。各フィールドの意味:
- action: BUY / SELL / HOLD
- confidence: 0〜1。今この瞬間に action を執行することへの確信度
- entry_type: MARKET（成行で今入る）/ LIMIT / CONDITIONAL（今は入らず conditional_plan を待つ）/ null
- entry_price, stop_loss, take_profit: BUY/SELL の場合は必須。stop_loss は analysis.invalidation の価格と一致させる
- analysis.macro_context: 環境認識（4H/1H）
- analysis.order_flow: 注文の攻防（15M/5M）
- analysis.invalidation: 無効化ラインの価格と、その価格である理由
- analysis.conflicts: 反対材料
- conditional_plan: 待ち条件がある場合のみ。wait_for / then_action / invalidate_if / expires_at（"YYYY-MM-DD HH:MM:SS UTC"）に加え、プログラムが機械的に判定するための構造化条件を必ず埋める:
  - trigger_price + trigger_condition（CLOSE_ABOVE / CLOSE_BELOW）: 5M確定足の終値がこの価格をこの向きに抜けたら成立
  - invalidate_price + invalidate_condition: 5M確定足の終値がこの価格をこの向きに抜けたら破棄
  - stop_loss / take_profit: 成立時に使う値
  条件を1つの価格で表せない場合は、最も本質的な1条件に絞って書く
- observed: 各画像のタイトルに書かれている "last bar" の時刻を "YYYY-MM-DD HH:MM" 形式でそのまま転記
- reasoning: 判断に至った論理の要約
"#
        )
    }
}

fn absolute(p: &Path) -> String {
    std::fs::canonicalize(p)
        .unwrap_or_else(|_| {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                std::env::current_dir().map(|d| d.join(p)).unwrap_or_else(|_| p.to_path_buf())
            }
        })
        .to_string_lossy()
        .to_string()
}
