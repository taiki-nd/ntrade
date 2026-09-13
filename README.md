# ntrade: LLM駆動型 FX自動売買システム

主要通貨ペア（USD/JPY, EUR/USD）を対象とし、マルチタイムフレーム（5分足〜4時間足）の市場データ（数値指標・チャート画像）を元に、LLM（Gemini / Claude / ChatGPT等）の推論を活用して自動売買を行うローカル運用型のトレードシステムです。

- **バックエンド**: Rust 1.98 (Tokio + Axum + cTrader Open API + plotters + SQLite)
- **フロントエンド**: Next.js 16 (App Router) + React 19 + Tailwind CSS v4 + shadcn/ui
- **推論パイプライン**: ローカル CLI 連携 (`claude -p` / `agy -p`)

---

## クイックスタート (Makefile)

プロジェクトルートから以下の `make` コマンドで起動・操作が可能です。

```bash
# コマンド一覧を表示
make help

# 1. フロントエンド（Next.js 管理画面）のみ起動 (http://localhost:3000)
make ui

# 2. Rust コアエンジンのみ起動
make engine

# 3. フロントエンドとRustエンジンを並行起動
make dev

# 4. LLM CLI推論 (claude -p) のPoCを実行
make poc

# 5. Rust 単体テストの実行
make test

# 6. プロジェクト全体のビルド (Rust + Next.js)
make build

# 7. 依存パッケージの再インストール
make install

# 8. ビルドキャッシュのクリーンアップ
make clean
```

---

## ドキュメント構成

詳細は [`docs/`](./docs) ディレクトリをご参照ください。

- [01. システム全体アーキテクチャ](./docs/01_system_architecture.md)
- [02. プライスアクション特化型 取引戦略 & プロンプト設計](./docs/02_trading_strategy.md)
- [03. cTrader Open API 連携仕様](./docs/03_ctrader_spec.md)
- [04. ロードマップ & 確定技術スタック](./docs/04_roadmap_and_tech_stack.md)
- [05. AI学習プロセス & プロンプト改善サイクル](./docs/05_ai_learning_and_prompt_tuning.md)
