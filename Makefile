SHELL := /bin/zsh
export PATH := $(HOME)/.nodenv/shims:$(PATH)

.PHONY: help install dev ui engine test poc ctrader build clean step4 replay-fetch replay-check replay-run replay-report replay-list

help: ## コマンド一覧を表示
	@echo "ntrade - LLM駆動型 FX自動売買システム"
	@echo ""
	@echo "使用可能なコマンド:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

install: ## 依存関係のインストール (フロントエンド pnpm)
	@echo "--> Installing dashboard dependencies..."
	cd dashboard && pnpm install

ui: ## 管理画面 (Next.js) を起動 (http://localhost:3000)
	@echo "--> Starting Next.js Dashboard on http://localhost:3000..."
	cd dashboard && pnpm dev -p 3000

engine: ## Rustコアエンジンを起動 (http://localhost:4000)
	@echo "--> Starting Rust Core Engine on http://localhost:4000..."
	cargo run --bin ntrade

dev: ## フロントエンドとRustエンジンを並行起動 (UI: 3000 / API: 4000)
	@echo "--> Starting ntrade (Dashboard on :3000 + Engine on :4000)..."
	@trap 'kill 0' SIGINT SIGTERM; \
	(cd dashboard && pnpm dev -p 3000) & \
	cargo run --bin ntrade & \
	wait

poc: ## Snapshot生成 (画像4枚+事実JSON) & claude -p 画像パス渡し推論のPoCを実行
	@echo "--> Running Snapshot & LLM inference PoC..."
	cargo run --bin poc_price_action

ctrader: ## cTrader Open API 接続とバーデータ取得のPoCを実行
	@echo "--> Running cTrader Open API PoC..."
	cargo run --bin poc_ctrader

step4: poc ## (旧名) poc のエイリアス

replay-fetch: ## ヒストリカルバーを cTrader から取得 (FROM=YYYY-MM-DD PAIR=USDJPY)
	cargo run --bin replay -- fetch --pair $(or $(PAIR),USDJPY) --from $(or $(FROM),2026-06-15)

replay-check: ## 未来漏れ検証 (Snapshot に t より後のデータが無いこと)
	cargo run --bin replay -- check-leak --pair $(or $(PAIR),USDJPY) --samples $(or $(SAMPLES),50)

replay-run: ## リプレイ実行 (FROM= TO= LIMIT= LABEL= STEP=)
	cargo run --bin replay -- run --pair $(or $(PAIR),USDJPY) --from $(FROM) --to $(TO) --step $(or $(STEP),15) $(if $(LIMIT),--limit $(LIMIT),) --label "$(LABEL)"

replay-report: ## リプレイ集計 (RUN=<id>)
	cargo run --bin replay -- report --run $(RUN)

replay-list: ## リプレイ run 一覧
	cargo run --bin replay -- list

test: ## テストを実行 (Rust単体テスト)
	@echo "--> Running Rust tests..."
	cargo test

build: ## 全体ビルド (Rust + Next.js)
	@echo "--> Building Rust engine..."
	cargo build
	@echo "--> Building Next.js dashboard..."
	cd dashboard && pnpm build

clean: ## ビルド成果物の削除
	@echo "--> Cleaning build artifacts..."
	cargo clean
	rm -rf dashboard/.next
