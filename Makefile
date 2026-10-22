SHELL := /bin/zsh
export PATH := $(HOME)/.local/bin:$(HOME)/.nodenv/shims:$(PATH)

.PHONY: help install dev ui engine test ctrader build clean bt-fetch bt-coverage bt-strategies bt-run bt-sweep bt-wf bt-list

help: ## コマンド一覧を表示
	@echo "ntrade - ロジック戦略による FX 自動売買システム"
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

ctrader: ## cTrader Open API 接続とバーデータ取得のPoCを実行
	@echo "--> Running cTrader Open API PoC..."
	cargo run --bin poc_ctrader

BT := cargo run --release --bin backtest --

bt-fetch: ## ヒストリカルバーを cTrader から取得 (PAIRS="USDJPY_z EURUSD_z" FROM=YYYY-MM-DD)
	$(BT) fetch $(foreach p,$(or $(PAIRS),USDJPY_z),--pair $(p)) --from $(or $(FROM),2023-01-01)

bt-coverage: ## DB にある足の範囲
	$(BT) coverage

bt-strategies: ## 使える戦略とパラメータ
	$(BT) strategies

bt-run: ## バックテスト (PAIR= FROM= TO= STRATEGY= TF= PARAMS="fast=20 slow=50" LABEL=)
	$(BT) run --pair $(or $(PAIR),USDJPY_z) --from $(FROM) $(if $(TO),--to $(TO),) --strategy $(or $(STRATEGY),ema_cross) --tf $(or $(TF),M15) $(foreach p,$(PARAMS),--param $(p)) --label "$(LABEL)" --trades 10

bt-sweep: ## グリッドサーチ (... GRID="fast=10..30:5 slow=50,100" OBJ=pf)
	$(BT) sweep --pair $(or $(PAIR),USDJPY_z) --from $(FROM) $(if $(TO),--to $(TO),) --strategy $(or $(STRATEGY),ema_cross) --tf $(or $(TF),M15) $(foreach g,$(GRID),--grid $(g)) --objective $(or $(OBJ),pf) --label "$(LABEL)"

bt-wf: ## ウォークフォワード (... GRID= TRAIN=180 TEST=60)
	$(BT) walk-forward --pair $(or $(PAIR),USDJPY_z) --from $(FROM) $(if $(TO),--to $(TO),) --strategy $(or $(STRATEGY),ema_cross) --tf $(or $(TF),M15) $(foreach g,$(GRID),--grid $(g)) --objective $(or $(OBJ),pf) --train-days $(or $(TRAIN),180) --test-days $(or $(TEST),60) --label "$(LABEL)"

bt-list: ## 保存したバックテスト結果の一覧
	$(BT) list

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
