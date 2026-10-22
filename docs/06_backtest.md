# 06. バックテスト

本番と同じ戦略（`strategy::build`）・同じリスクチェック（`guard::evaluate`）・同じロット計算を、SQLite に保存した過去の足で動かします。
CLI は `bin/backtest`（`make bt-*`）、管理画面の「バックテスト」タブからも単発の実行と結果の閲覧ができます。

## 1. データ

```bash
cargo run --release --bin backtest -- fetch --pair USDJPY_z --pair EURUSD_z --from 2023-01-01   # 既定で 1M,5M,15M,1H,4H,1D
cargo run --release --bin backtest -- coverage
```

- cTrader Open API の足（Bid）を `bars_history`（pair, period, ts）に保存する。DB に無い前側・後ろ側の期間だけを取り、30 日（1 分足）/ 180 日ごとに保存するので、途中で切れても続きから取れる
- 銘柄仕様（lot サイズ・pip 位置・最小発注量）も `api_cache` に保存し、バックテストのロット計算に使う（本番エンジンも接続時に保存する）
- 口座通貨と建値通貨が違う銘柄（JPY 口座の EURUSD など）は、換算用の銘柄（USDJPY_z）の足から各時点のレートを取る。無ければ `fx_rate` の固定レート

## 2. 進め方

1. 執行用の足を 1 本ずつ進める。執行用の足は既定で「期間をカバーする 1 分足があればそれ、無ければ戦略の時間足」（`exec_timeframe`）
2. 戦略の時間足が確定した時刻になったら、その時点で確定している足（`lookback` 本）だけを戦略に渡す。助走の足は期間の前から読む。助走が足りない間は判断しない
3. 新規注文はリスクチェックとロット計算を通してシミュレーターに出す（価格は銘柄の桁数に丸める）
4. 期間の終わりに残った建玉は最後の終値で決済する（`END_OF_TEST`）

## 3. 約定モデル（`src/backtest/sim.rs`）

足は Bid。Ask = Bid + スプレッド。買いは Ask で建てて Bid で手仕舞い、売りは Bid で建てて Ask で手仕舞う。
1 本の足の中の順序は分からないので、迷ったら不利な側に倒す。

| 場面 | 扱い |
| :--- | :--- |
| 成行 | 判断した次の足の始値 + 滑り |
| 指値 | 価格に届いたら指値の価格で約定（有利な窓開けは見込まない） |
| 逆指値 | 価格を抜けたら約定。窓を空けたら始値。さらに滑りを足す |
| 予約注文の期限 | 期限ちょうどに始まる足では約定させない |
| SL と TP に同じ足で届いた | SL |
| 予約注文が約定した足 | TP は見ない（約定前の値動きかもしれない）。SL は見る |
| 窓を空けて SL を越えた | 始値 + 滑りで決済 |
| TP | TP の価格で決済 |
| 戦略の決済シグナル | 次の足の始値 + 滑り |

## 4. コスト（`config/backtest.toml`）

| 項目 | 既定 | 内容 |
| :--- | :--- | :--- |
| `spread_pips` | 0.2 | ゼロスプレッド口座でも指標時などは開くので少し大きめ |
| `slippage_pips` | 0.1 | 成行・逆指値・SL の滑り（不利な方向） |
| `commission_per_lot` | 800（USDJPY_z）/ 700（EURUSD_z） | 1 lot・片道、口座通貨。実績の取引（0.01 lot の往復で約 14〜16 円）から推定 |
| `swap_long_per_lot` / `swap_short_per_lot` | 0 | 1 lot・1 日。`rollover_hour_utc`（21 時）をまたいだ建玉に付ける。土日は無し、水曜は 3 日分 |

銘柄ごとの上書きは `[pairs.<銘柄>]`。CLI の `--spread` / `--commission` / `--balance` で 1 回分だけ上書きできる。

## 5. 成績指標（`src/backtest/metrics.rs`）

取引数・勝率（純損益が正）・純損益・総利益/総損失・PF・期待値・平均利益/損失・ペイオフ・pips・最大ドローダウン（含み損益込みの資産で測る、金額と %）・リターン %・シャープレシオ（日次リターン、年率 252 日）・リカバリーファクター・最大連勝/連敗・平均保有時間・手数料/スワップ合計・売買別の件数と勝率・決済理由の内訳・月別損益。
各取引は MAE / MFE（保有中の最大逆行・順行 pips）も持つ。

## 6. グリッドサーチとウォークフォワード（`src/backtest/optimize.rs`）

```bash
# 全組み合わせを並列に走らせてスコア順に並べる
backtest sweep --pair USDJPY_z --from 2024-01-01 --strategy ema_cross --tf M15 \
  --grid fast=10..30:5 --grid slow=50,100,200 --objective pf --min-trades 30

# 学習期間で選んだパラメータを直後の検証期間で評価し、検証期間だけをつないだ成績を出す
backtest walk-forward --pair USDJPY_z --from 2024-01-01 --strategy ema_cross --tf M15 \
  --grid fast=10..30:5 --grid slow=50,100 --train-days 180 --test-days 60
```

- `--grid` は `name=start..end:step` / `name=v1,v2,...` / `name=v`
- 目的関数（`--objective`）: `net` / `pf` / `sharpe` / `recovery` / `expectancy`。取引数が `--min-trades` 未満の組み合わせは選ばない
- グリッドサーチだけで選んだパラメータは過去に合わせ込んだもの。採用の判断はウォークフォワードの検証期間の成績で行う

## 7. 保存

結果は `backtest_runs`（条件・成績・グリッドサーチの全結果・ウォークフォワードの窓ごとの結果）、`backtest_trades`、`backtest_equity`（間引いた資産の推移）に保存し、`backtest list` / `show` と管理画面で見られる。

## 8. 本番との違い（既知の限界）

- 1 銘柄ずつ走らせる。複数銘柄をまとめた口座全体の建玉上限・日次損失は再現しない
- スプレッドは一定。実際の指標時の拡大は `max_spread_pips` のチェックにも反映されない
- 証拠金・レバレッジのチェックは本番だけ
- 1 分足の中の値動きは分からないので、1 分足でも同じ足の中の SL/TP の順序は不利側に倒している
