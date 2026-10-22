# 01. システム全体アーキテクチャ

## 1. 全体像

```
┌─────────────── Next.js 管理画面 (localhost:3000) ───────────────┐
│ 概要 / シグナル / チャート / 取引履歴 / バックテスト / 設定        │
└───────────────────────────┬─────────────────────────────────────┘
                            │ REST (localhost:4000)
┌───────────────────────────┴──────────── Rust コアエンジン ───────┐
│ scheduler ─ 1 分ごとに、足が確定したデプロイを並行に実行            │
│   └ live::run_deployment                                        │
│       確定足（cTrader）→ strategy.on_bar → guard::evaluate       │
│       → ロット計算・証拠金確認 → executor（cTrader に発注）         │
│       → signals テーブルに記録                                   │
│ reconcile ─ 60 秒ごとにブローカーの建玉・予約注文と照合            │
│ balance history ─ 残高の推移を取り込む                            │
│ server ─ Axum の REST API（管理画面・バックテストの実行）           │
└───────────────────────────┬─────────────────────────────────────┘
                            │ SQLite (data/ntrade.db)
        bars_history / signals / positions / working_orders / trades
        backtest_runs / backtest_trades / backtest_equity / app_settings

┌──────────────── bin/backtest（CLI） ────────────────┐
│ fetch → bars_history                                │
│ run / sweep / walk-forward                          │
│   SQLite の足 → strategy.on_bar → guard::evaluate     │
│   → 約定シミュレーター → 成績 → backtest_* に保存      │
└────────────────────────────────────────────────────┘
```

## 2. モジュール

| モジュール | 役割 |
| :--- | :--- |
| `strategy` | `Strategy` トレイトと戦略の実装・一覧（`catalog` / `build`）。入出力の型（`OrderIntent` / `Action` など） |
| `indicators` | EMA・SMA・ATR・RSI・チャネル・スイング・時間足のまとめ直しなど。戦略とリスクチェックが共通で使う |
| `guard` | リスクチェック（SL/TP の向き・RR・SL 幅・建玉数・日次損失・スプレッド・指標前後）とロット計算 |
| `backtest` | 約定シミュレーター（`sim`）・成績指標（`metrics`）・データ読み込み（`data`）・グリッドサーチとウォークフォワード（`optimize`）・設定（`config`） |
| `live` | 本番の 1 デプロイ分の実行（足の取得 → 戦略 → リスクチェック → 発注 → シグナル記録） |
| `scheduler` | 足の確定時刻 + 待ち秒数ごとに `live` を回し、終わったら照合する |
| `executor` | 発注の抽象（`OrderSink`）と cTrader 実装（成行・予約注文・取消・SL/TP 変更） |
| `ctrader` | cTrader Open API（認証・銘柄仕様・足・気配値・発注・照合・決済履歴） |
| `server` | REST API と状態（`AppState`）。建玉・予約注文の照合と決済の取り込み |
| `storage` | SQLite（足・シグナル・建玉・取引・残高・バックテスト結果・設定・トークン） |
| `settings` | 画面から変更する取引設定（監視銘柄・待ち秒数・リスク設定・戦略のデプロイ） |
| `instrument` | 銘柄仕様（pip 位置・lot サイズ・発注量の刻み・取引時間） |
| `chart` | チャート画像（管理画面のチャートタブ） |

## 3. 戦略のデプロイ

本番で動かす戦略は、取引設定の `deployments` に 1 件ずつ書きます（管理画面の設定から編集）。

```json
{ "id": "ema-usdjpy", "pair": "USDJPY_z",
  "strategy": { "name": "ema_cross", "timeframe": "M15", "params": { "fast": 20, "slow": 50 } },
  "enabled": true, "dryRun": true }
```

- `enabled` が true のものだけ動かす。`dryRun` が true の間は発注せず、シグナルログに「DRY_RUN」として残すだけ
- 戦略は時間足の確定ごと（M15 なら毎時 0/15/30/45 分、UTC 0:00 起点）に、`bar_delay_secs` 待ってから呼ばれる
- 戦略が見る建玉・予約注文は、そのデプロイが建てたもの（`strategy` 列がデプロイ ID）だけ。手動や取り込みの建玉は戦略から見えないが、リスクチェックの建玉数には数える
- 発注の comment に `sig-<ミリ秒>-<n>@<デプロイ ID>` を入れるので、再起動やブローカー側からの取り込みでも持ち主が分かる

## 4. 照合と決済

SL/TP はブローカー側に置き、約定・決済はブローカーが判定します。ntrade は 60 秒ごと（と各サイクルの後）に照合し、

- 消えた予約注文 → 約定・取消・期限切れを判定。約定なら建玉にシグナルと戦略を引き継ぐ
- 消えた建玉 → ブローカーの決済約定（手数料・スワップ込みの純損益）から取引履歴に記録
- ブローカーにだけある建玉・注文 → 取り込む（comment に ntrade のタグがあれば持ち主も復元）

戦略の決済シグナル（`ClosePosition`）は ntrade が成行で決済し、決済理由を `STRATEGY` として記録します。

## 5. 当日損益

日次損失の上限（`daily_loss_limit_pct`）と管理画面の当日損益は、取引履歴のうち UTC の当日に決済したものから求めます（再起動や日付の変わり目でもずれない）。
