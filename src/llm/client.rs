use crate::strategy::types::TradeDecision;
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// LLM 推論バックエンドの抽象。CLI 実装と将来の API 直叩き実装を差し替え可能にする。
pub trait LlmBackend: Send + Sync {
    fn infer<'a>(
        &'a self,
        prompt: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<TradeDecision>> + Send + 'a>>;
}

/// `claude -p` サブプロセス推論クライアントの設定
#[derive(Debug, Clone)]
pub struct LlmClientConfig {
    pub cli_binary: String,
    /// 画像4枚の Read が各1ターン入るため、テキストのみの場合より長めに取る
    pub timeout_secs: u64,
    pub max_turns: u32,
    /// 許可するツール。画像を読むために Read が必要。
    pub allowed_tools: Vec<String>,
    /// None なら CLI のデフォルトモデル
    pub model: Option<String>,
}

impl Default for LlmClientConfig {
    fn default() -> Self {
        Self {
            cli_binary: "claude".to_string(),
            timeout_secs: 120,
            max_turns: 8,
            allowed_tools: vec!["Read".to_string()],
            model: None,
        }
    }
}

/// `claude -p` を使う LLM クライアント
pub struct LlmClient {
    config: LlmClientConfig,
}

impl LlmClient {
    pub fn new(config: LlmClientConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &LlmClientConfig {
        &self.config
    }

    /// プロンプトを CLI に流し込み、TradeDecision を取得。失敗時は HOLD に倒す。
    pub async fn infer(&self, prompt: &str) -> Result<TradeDecision> {
        info!(
            cli = %self.config.cli_binary,
            timeout = self.config.timeout_secs,
            "Starting LLM inference via CLI"
        );

        let fut = self.execute_cli(prompt);
        match timeout(Duration::from_secs(self.config.timeout_secs), fut).await {
            Ok(Ok(raw)) => Ok(self.parse_decision(&raw)),
            Ok(Err(e)) => {
                error!("CLI execution failed: {e}");
                Ok(TradeDecision::fallback_hold(format!("CLI execution error: {e}")))
            }
            Err(_) => {
                warn!("LLM inference timed out after {}s", self.config.timeout_secs);
                Ok(TradeDecision::fallback_hold("inference timeout"))
            }
        }
    }

    /// 任意の JSON Schema で構造化出力を得る（自己反省など TradeDecision 以外の用途）
    pub async fn infer_json(&self, prompt: &str, schema: serde_json::Value) -> Result<serde_json::Value> {
        let schema_text = schema.to_string();
        let fut = self.execute_cli_with_schema(prompt, &schema_text);
        let raw = timeout(Duration::from_secs(self.config.timeout_secs), fut)
            .await
            .map_err(|_| anyhow!("inference timeout"))??;
        Self::extract_decision_json(&raw)
    }

    async fn execute_cli(&self, prompt: &str) -> Result<String> {
        let schema = TradeDecision::json_schema().to_string();
        self.execute_cli_with_schema(prompt, &schema).await
    }

    async fn execute_cli_with_schema(&self, prompt: &str, schema: &str) -> Result<String> {
        let binary_path = resolve_cli_path(&self.config.cli_binary);
        let mut cmd = Command::new(binary_path);
        cmd.arg("-p")
            .arg("--output-format")
            .arg("json")
            .arg("--json-schema")
            .arg(schema)
            .arg("--max-turns")
            .arg(self.config.max_turns.to_string())
            .arg("--tools")
            .arg(self.config.allowed_tools.join(","));
        if let Some(model) = &self.config.model {
            cmd.arg("--model").arg(model);
        }

        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn {}", self.config.cli_binary))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await.context("Failed to write prompt")?;
            stdin.flush().await?;
            drop(stdin);
        }

        let output = child.wait_with_output().await.context("Failed to wait for CLI")?;
        if !output.status.success() {
            return Err(anyhow!(
                "CLI exited with {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        String::from_utf8(output.stdout).context("CLI output is not UTF-8")
    }

    /// CLI 出力（JSON エンベロープ or 生テキスト）から TradeDecision を取り出す
    pub fn parse_decision(&self, raw: &str) -> TradeDecision {
        match Self::extract_decision_json(raw).and_then(|v| {
            serde_json::from_value::<TradeDecision>(v).map_err(|e| anyhow!("schema mismatch: {e}"))
        }) {
            Ok(d) => {
                info!(action = ?d.action, confidence = d.confidence, "Parsed TradeDecision");
                d
            }
            Err(e) => {
                error!(error = %e, raw = %truncate(raw, 800), "Failed to parse TradeDecision");
                TradeDecision::fallback_hold(format!("parse error: {e}"))
            }
        }
    }

    /// 1. `--output-format json` のエンベロープ (`structured_output` → `result`)
    /// 2. 生JSON / Markdown コードブロック
    fn extract_decision_json(raw: &str) -> Result<serde_json::Value> {
        let trimmed = raw.trim();
        if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if envelope.get("type").and_then(|t| t.as_str()) == Some("result") {
                if envelope.get("is_error").and_then(|b| b.as_bool()) == Some(true) {
                    return Err(anyhow!(
                        "CLI reported error: {}",
                        envelope.get("result").and_then(|r| r.as_str()).unwrap_or("")
                    ));
                }
                if let Some(so) = envelope.get("structured_output") {
                    if so.is_object() {
                        return Ok(so.clone());
                    }
                }
                if let Some(result) = envelope.get("result").and_then(|r| r.as_str()) {
                    return Self::extract_from_text(result);
                }
                return Err(anyhow!("result envelope without structured_output/result"));
            }
            if envelope.is_object() {
                return Ok(envelope);
            }
        }
        Self::extract_from_text(trimmed)
    }

    fn extract_from_text(text: &str) -> Result<serde_json::Value> {
        let re = Regex::new(r"(?s)```(?:json)?\s*(\{.*?\})\s*```").unwrap();
        if let Some(c) = re.captures(text).and_then(|c| c.get(1)) {
            return serde_json::from_str(c.as_str()).context("invalid JSON in code block");
        }
        if let (Some(s), Some(e)) = (text.find('{'), text.rfind('}')) {
            if s < e {
                return serde_json::from_str(&text[s..=e]).context("invalid JSON object");
            }
        }
        Err(anyhow!("no JSON object found in output"))
    }
}

impl LlmBackend for LlmClient {
    fn infer<'a>(
        &'a self,
        prompt: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<TradeDecision>> + Send + 'a>> {
        Box::pin(LlmClient::infer(self, prompt))
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// CLI バイナリのパス解決。
/// 1. 環境変数 `NTRADE_LLM_CLI` があれば最優先
/// 2. `~/.local/bin/{binary}` が存在すればそれを使用（古い nodenv 等の shim より優先）
/// 3. それ以外は指定された名前で PATH 検索
fn resolve_cli_path(binary: &str) -> std::path::PathBuf {
    if let Ok(custom) = std::env::var("NTRADE_LLM_CLI") {
        if !custom.trim().is_empty() {
            return std::path::PathBuf::from(custom.trim());
        }
    }
    if !binary.contains('/') {
        if let Ok(home) = std::env::var("HOME") {
            let local_bin = std::path::PathBuf::from(home).join(".local/bin").join(binary);
            if local_bin.is_file() {
                return local_bin;
            }
        }
    }
    std::path::PathBuf::from(binary)
}
