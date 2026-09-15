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
| [02_trading_strategy.md](./02_trading_strategy.md) | プライスアクション特化型戦略、プログラム=事実/LLM=解釈の役割分担、インプット設計（画像+生OHLC+客観数値）、プロンプト設計、事後ガード |
| [03_ctrader_spec.md](./03_ctrader_spec.md) | cTrader Open API仕様、認証、注文執行、サーバーサイドSL/TP、切断耐性 |
| [04_roadmap_and_tech_stack.md](./04_roadmap_and_tech_stack.md) | 確定技術スタック（Rust + Next.js）、開発ロードマップ、設計方針の転換（2026-09-16） |
| [05_ai_learning_and_prompt_tuning.md](./05_ai_learning_and_prompt_tuning.md) | AI学習の定義（リプレイ評価・較正・教訓）、過学習防止 |
| [06_replay_environment.md](./06_replay_environment.md) | リプレイ環境：ヒストリカル取得、未来漏れ防止、サンプリング、採点、集計 |

## 設計の3原則（2026-09-16 確定）
1. **プログラムは客観的事実のみを渡す**: 時間足ごとの画像4枚、15M/5Mの生OHLC、ATR・前日高安・キリ番・スイング価格リスト。パターン判定・トレンドラベルは渡さない。
2. **プログラムの役割は事後ガードと条件執行**: LLM出力の機械的検証と、LLMが返した条件付きプランの監視・発注。入力前に相場観を絞らない。
3. **リプレイ環境を本番稼働より先に用意する**: 過去スナップショットでLLM判断を採点し、確信度閾値やSL幅をデータで較正する。
