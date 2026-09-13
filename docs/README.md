# ntrade: LLM駆動型 FX自動売買システム 設計ドキュメント

## プロジェクト概要
本プロジェクトは、主要通貨ペア（USD/JPY, EUR/USD、将来的にXAU/USD）を対象とし、マルチタイムフレーム（5分足〜4時間足）の市場データ（数値指標・チャート画像）を元に、LLM（Gemini / Claude / ChatGPT等）の推論を活用して自動売買を行うローカル運用型のトレードシステムです。

外部SaaSサービス化は行わず、手元のローカル環境で常駐する **Rust コアエンジン（Tokio + Axum）** と、操作・監視を行う **Next.js 管理画面（next-scaffoldベース）** で構成されます。
推論エンジンにはローカルCLIツール（`claude -p` や `agy -p` 等のパイプライン）を利用し、ブローカー接続・注文執行には **cTrader Open API** を利用します。

---

## ドキュメント構成

| ファイル名 | 概要 |
| :--- | :--- |
| [01_system_architecture.md](./01_system_architecture.md) | システム全体アーキテクチャ（Rustエンジン + Next.js管理画面 + CLI推論） |
| [02_trading_strategy.md](./02_trading_strategy.md) | プライスアクション特化型戦略、3大トリガー、客観的SL、プロンプト設計 |
| [03_ctrader_spec.md](./03_ctrader_spec.md) | cTrader Open API仕様、認証、注文執行、サーバーサイドSL/TP、切断耐性 |
| [04_roadmap_and_tech_stack.md](./04_roadmap_and_tech_stack.md) | 確定技術スタック（Rust + Next.js）、開発ロードマップ |
| [05_ai_learning_and_prompt_tuning.md](./05_ai_learning_and_prompt_tuning.md) | AI学習プロセス、Few-shot作成、自己反省（Self-Reflection）改善ループ |
