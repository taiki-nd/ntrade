# ntrade: ロジック戦略による CFD・FX 自動売買システム

FX・貴金属（XAUUSD など）・株価指数の CFD を取引する、ローカル運用型の自動売買システムです。
売買判断はすべて Rust で書いたロジック戦略が行い、**バックテストと本番で同じ戦略コード・同じリスクチェック**を通します。
ブローカー接続・発注は cTrader Open API です。

- **バックエンド**: Rust (Tokio + Axum + cTrader Open API + SQLite + plotters)
- **フロントエンド**: Next.js 16 (App Router) + React 19 + Tailwind CSS v4 + shadcn/ui

```
戦略（src/strategy）── 確定足 → 注文の意図（成行・指値・逆指値 / 決済 / 取消 / SL・TP 変更）
   │
   ├─ バックテスト（src/backtest）: SQLite の過去の足 → 約定シミュレーター → 成績・資産推移
   └─ 本番（src/live + scheduler）: cTrader の確定足 → リスクチェック → cTrader へ発注 → 照合
```

---

## クイックスタート (Makefile)

```bash
make help            # コマンド一覧

# 過去データの取得（cTrader の認証情報が必要。DB に無い期間だけ取る）
make bt-fetch PAIRS="USDJPY_z EURUSD_z" FROM=2023-01-01
make bt-coverage

# バックテスト
make bt-strategies
make bt-run   PAIR=USDJPY_z FROM=2024-01-01 STRATEGY=ema_cross TF=M15 PARAMS="fast=20 slow=50"
make bt-sweep PAIR=USDJPY_z FROM=2024-01-01 STRATEGY=ema_cross TF=M15 GRID="fast=10..30:5 slow=50,100,200"
make bt-wf    PAIR=USDJPY_z FROM=2024-01-01 STRATEGY=ema_cross TF=M15 GRID="fast=10..30:5 slow=50,100" TRAIN=180 TEST=60
make bt-list

# 本番エンジンと管理画面
make dev             # UI (http://localhost:3000) + エンジン (http://localhost:4000)
make test            # Rust のテスト
```

戦略を本番で動かすには、管理画面の設定で「戦略のデプロイ」を追加します（既定はドライラン = 発注せずシグナルを記録するだけ）。

---

## ドキュメント

- [01. システム全体アーキテクチャ](./docs/01_system_architecture.md)
- [02. 戦略の書き方とリスク管理](./docs/02_trading_strategy.md)
- [03. cTrader Open API 連携仕様](./docs/03_ctrader_spec.md)
- [04. ロードマップ & 技術スタック](./docs/04_roadmap_and_tech_stack.md)
- [06. バックテスト](./docs/06_backtest.md)
