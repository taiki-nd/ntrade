# 02. 戦略の書き方とリスク管理

## 1. 戦略のインターフェース（`src/strategy/types.rs`）

```rust
pub trait Strategy: Send {
    /// 判断に使う足の本数（指標の助走を含む）
    fn lookback(&self) -> usize;
    /// 戦略の時間足が確定するたびに呼ばれる
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Action>;
}

pub struct StrategyContext<'a> {
    pub pair: &'a str,
    pub now: DateTime<Utc>,          // 直近の足が確定した時刻
    pub bars: &'a [CandleBar],       // 確定足（古い順。末尾が今確定した足。lookback 本まで）
    pub pip_size: f64,
    pub positions: &'a [PositionView], // この戦略の建玉
    pub orders: &'a [OrderView],       // この戦略の予約注文
}

pub enum Action {
    Open(OrderIntent),                                   // 新規（成行 / 指値 / 逆指値）
    ClosePosition { id, reason },                        // 成行で決済
    CancelOrder { id, reason },                          // 予約注文の取消
    ModifyPosition { id, stop_loss, take_profit, reason }, // SL / TP の付け替え
}

pub struct OrderIntent {
    pub side: Side,                 // Buy / Sell
    pub entry: Entry,               // Market / Limit { price } / Stop { price }
    pub stop_loss: f64,             // 必須（絶対価格）
    pub take_profit: Option<f64>,   // None なら置かない
    pub expiry_minutes: Option<u32>,// 予約注文の期限（None なら上限まで）
    pub reason: String,             // シグナルログ・取引履歴に残る
}
```

戦略が決めるのは「どの向きに・どこで入り・どこで損切り/利確するか」まで。**発注量はリスク設定から呼び出し側が決めます**（戦略を変えずにリスクだけ変えられるように）。

### 守ること
- **判断は渡された足だけから決める**。本番は再起動のたびに戦略を作り直すので、内部状態に頼るとバックテストと本番で結果がずれる。EMA のように始点の影響が残る指標は、`lookback` を十分に長くする（本番もバックテストも同じ本数を渡す）
- 成行は「足が確定した後」に出るので、約定は次の足の始値付近になる（バックテストも同じ）
- 損切り（`stop_loss`）は必須。ブローカー側に置くので、エンジンが止まっても建玉は守られる

## 2. 戦略の追加手順

1. `src/strategy/<name>.rs` に `NAME`・`info()`（パラメータの定義）・`Strategy` の実装を書く（`ema_cross.rs` が雛形）
2. `src/strategy/mod.rs` の `catalog()` と `build()` に 1 行ずつ足す
3. テストを書く（合成した足で、期待どおりの `Action` が出るか）
4. `make bt-run` / `make bt-sweep` / `make bt-wf` で検証する
5. 管理画面でデプロイを追加（ドライラン）→ シグナルログで確認 → 発注に切り替え

パラメータは `ParamSpec`（既定値・範囲・整数か）で定義し、`build` の前に `resolve_params` が既定値の補完・範囲チェック・整数への丸めをする。

## 3. 同梱の戦略（動作確認用のわかりやすいもの）

### `ema_cross` — EMA クロス
- 短期 EMA（`fast`）が長期 EMA（`slow`）を上抜けたら買い、下抜けたら売り（成行）
- SL は終値から ATR(`atr_period`) × `sl_atr`、TP は SL 幅 × `tp_rr`（0 で TP なし）
- `exit_on_cross = 1` なら反対のクロスで決済してドテン。同じ向きの建玉があれば積み増さない

### `donchian_breakout` — ドンチャン・ブレイクアウト
- 直近 `channel` 本の高値 + `buffer_pips` に買いの逆指値、安値 - `buffer_pips` に売りの逆指値（1 足ごとに置き直す。期限 1 足）
- EMA(`trend_ema`) より上なら買いだけ・下なら売りだけ（0 で両方向）
- SL は ATR × `sl_atr`、TP は SL 幅 × `tp_rr`（既定 0 = TP なし）
- 保有中は反対側の `exit_channel` 本のチャネルを終値で抜けたら決済

### 米国株価指数 CFD 用（30 分足・NY の現物時間で判断）
`src/strategy/us_cash.rs` が 30 分足を NY 9:30〜16:00 の日ごとにまとめる（夏時間も NY 時刻で扱う）。どちらも M30 で動かす。

#### `noise_area` — ノイズ帯ブレイクアウト（Zarattini, Aziz & Barbon 2024）
- 帯: 上 = max(始値, 前日引け) × (1 + σ)、下 = min(始値, 前日引け) × (1 − σ)。σ は同じ時刻の |価格 / 始値 − 1| の直近 `lookback_days` 日の平均
- 10:00〜15:30 の 30 分ごとに、帯の上なら買い・下なら売り（成行）。帯の内側に戻ったら決済、16:00 に必ず手仕舞い（日をまたがない）
- SL は ATR × `sl_atr`（途中の急変への備え）
- 検証（2016-06〜2026-10、30 分足）: US100 で 1 単位あたりの Sharpe 0.9 前後（コスト約 1bp/往復）。US500 は 0.5 程度で弱い
- 既知の限界: 短縮取引の日（感謝祭の翌日など）に最後の判断で建てると、次の足が週明けになり持ち越すことがある

#### `rsi2_dip` — RSI(2) 押し目買い
- 16:00 の引けで日足 RSI(`rsi_period`) が `threshold` 未満なら買い（成行）。次の日以降の引けで RSI が閾値以上なら決済、未満なら持ち続ける
- SL は ATR × `sl_atr`（既定 12）。狭くすると押し目の途中で切られて優位性が消える（M30 ATR の 5 倍だと SL 到達 3 割・期待値半減）
- 日をまたぐのでスワップ（買い 年率 −5.45% ≒ 1.5bp/日）が掛かる
- 既定の SL 幅は、リスク設定の `sl_atr_max`（1H ATR の 4 倍）を超えることが多い

## 4. リスクチェック（`src/guard`）

新規注文（`Action::Open`）は、バックテストでも本番でも `guard::evaluate` を通ってから発注します。弾いたチェック名はバックテストの集計（`rejected by:`）と本番のシグナルログ（`guardResult`）に残ります。

| チェック | 内容 | 設定 |
| :--- | :--- | :--- |
| `SL_WRONG_SIDE` / `TP_WRONG_SIDE` | SL・TP が建値の正しい側にあるか | — |
| `RR_TOO_LOW` | (TP - 建値) / (建値 - SL)。TP を置く注文だけ | `min_rr` |
| `SL_TOO_TIGHT` | SL 幅の下限（pips、FX のみ） | `sl_min_pips` |
| `SL_ATR_RANGE` | SL 幅が 5M ATR × `sl_atr_min` 以上、`sl_atr_max_tf` の ATR × `sl_atr_max` 以下（0 で見ない） | `sl_atr_*` |
| `ORDER_PRICE_SIDE` | 指値・逆指値が現在値の正しい側にあるか | — |
| `ORDER_EXPIRY` | 予約注文の期限が上限以内か | `order_max_hours` |
| `SPREAD_TOO_WIDE` | スプレッド上限（pips / ATR 比） | `max_spread_pips` / `max_spread_atr` |
| `MAX_POSITIONS` | 建玉 + 予約注文の数（銘柄ごと・全体） | `max_positions_per_pair` / `max_positions_total` |
| `DAILY_LOSS_LIMIT` | 当日（UTC）の確定損益が上限を超えたら新規停止 | `daily_loss_limit_pct` |
| `NEWS_BLACKOUT` | 指標発表の前後 | `news_blackout` |

本番だけのチェック: 証拠金使用率・実効レバレッジ（`max_margin_usage_pct` / `max_effective_leverage`。ブローカーに必要証拠金を問い合わせる）。

### ロット計算
- `risk_pct` を設定すると、残高 × `risk_pct`% を SL 幅（pips）× 1 lot・1 pip の価値で割ったロット（刻みに切り捨て）。設定しなければ `fixed_volume_lots`
- 1 pip の価値は、本番はブローカーの銘柄仕様と直近レート、バックテストは保存した銘柄仕様と換算用の足から求める
- 最小発注量に届かなければ発注しない（切り上げると許容リスクを超えるため）

リスク設定の値は SQLite（`app_settings`）に保存し、管理画面から変更する。初回だけ `config/guard.toml` を初期値にする。バックテストは既定で同じ保存値を使う（`config/backtest.toml` の `guard`）。
