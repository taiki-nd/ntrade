SHELL := /bin/zsh
export PATH := $(HOME)/.nodenv/shims:$(PATH)

.PHONY: help install dev ui engine test poc build clean

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

poc: ## LLM CLI推論 (claude -p) のPoCを実行
	@echo "--> Running LLM CLI inference PoC..."
	cargo run --bin poc_inference

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
