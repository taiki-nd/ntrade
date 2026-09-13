use crate::strategy::types::{Action, PriceActionAnalysis, TradeDecision};
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// LLM CLI 推論クライアント設定
#[derive(Debug, Clone)]
pub struct LlmClientConfig {
    pub cli_binary: String, // "claude" or "agy"
    pub timeout_secs: u64,
}

impl Default for LlmClientConfig {
    fn default() -> Self {
        Self {
            cli_binary: "claude".to_string(),
            timeout_secs: 45,
        }
    }
}

/// LLM CLI 推論クライアント
pub struct LlmClient {
    config: LlmClientConfig,
}

impl LlmClient {
    pub fn new(config: LlmClientConfig) -> Self {
        Self { config }
    }

    /// プロンプトをCLIサブプロセス（claude -p / agy -p）へ流し込み、TradeDecisionを取得
    pub async fn infer(&self, prompt: &str) -> Result<TradeDecision> {
        info!(
            cli = %self.config.cli_binary,
            timeout = self.config.timeout_secs,
            "Starting LLM inference via CLI pipeline..."
        );

        let infer_future = self.execute_cli_subcommand(prompt);
        let duration = Duration::from_secs(self.config.timeout_secs);

        match timeout(duration, infer_future).await {
            Ok(result) => match result {
                Ok(raw_output) => self.parse_decision(&raw_output),
                Err(e) => {
                    error!("CLI command execution failed: {e}");
                    Ok(self.fallback_hold(&format!("CLI execution error: {e}")))
                }
            },
            Err(_) => {
                warn!("LLM inference timed out after {}s", self.config.timeout_secs);
                Ok(self.fallback_hold("Inference timeout, defaulting to safe HOLD"))
            }
        }
    }

    /// サブプロセスを実行し、標準出力を取得
    async fn execute_cli_subcommand(&self, prompt: &str) -> Result<String> {
        let mut child = Command::new(&self.config.cli_binary)
            .arg("-p")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn {}", self.config.cli_binary))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .await
                .context("Failed to write prompt to stdin")?;
            stdin.flush().await.context("Failed to flush stdin")?;
            drop(stdin); // EOFを送信して完了を促す
        }

        let output = child
            .wait_with_output()
            .await
            .context("Failed to wait for child process output")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "CLI process exited with code {:?}: {}",
                output.status.code(),
                stderr
            ));
        }

        let stdout = String::from_utf8(output.stdout)
            .context("Output contains invalid UTF-8 characters")?;
        Ok(stdout)
    }

    /// 出力テキストからJSONを抽出してTradeDecisionにデシリアライズ
    pub fn parse_decision(&self, raw: &str) -> Result<TradeDecision> {
        // ```json ... ``` ブロックを抽出
        let json_str = if let Some(extracted) = Self::extract_json_block(raw) {
            extracted
        } else {
            // ブロックが無い場合は全体をJSONとしてパース試行
            raw.trim().to_string()
        };

        match serde_json::from_str::<TradeDecision>(&json_str) {
            Ok(decision) => {
                info!(
                    action = ?decision.action,
                    confidence = decision.confidence,
                    "Successfully parsed TradeDecision from LLM"
                );
                Ok(decision)
            }
            Err(e) => {
                error!(
                    error = %e,
                    raw_json = %json_str,
                    "Failed to parse LLM JSON into TradeDecision"
                );
                Ok(self.fallback_hold(&format!("JSON parse error: {e}")))
            }
        }
    }

    /// MarkdownコードブロックからJSON文字列を抽出
    fn extract_json_block(text: &str) -> Option<String> {
        let re = Regex::new(r"(?s)```(?:json)?\s*(\{.*?\})\s*```").ok()?;
        if let Some(captures) = re.captures(text) {
            if let Some(matched) = captures.get(1) {
                return Some(matched.as_str().trim().to_string());
            }
        }

        // 最も外側の波括弧を探すフォールバック
        if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
            if start < end {
                return Some(text[start..=end].to_string());
            }
        }

        None
    }

    /// 安全装置としての見送り（HOLD）フォールバック
    fn fallback_hold(&self, reason: &str) -> TradeDecision {
        TradeDecision {
            action: Action::Hold,
            confidence: 0.0,
            entry_type: None,
            entry_price: None,
            stop_loss: None,
            take_profit: None,
            risk_reward_ratio: None,
            price_action_analysis: PriceActionAnalysis {
                macro_bias: "N/A (System Fallback)".to_string(),
                trigger_pattern: "N/A (System Fallback)".to_string(),
                invalidation_point: "N/A".to_string(),
            },
            reasoning: format!("FALLBACK_HOLD: {}", reason),
        }
    }
}
