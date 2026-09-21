//! FIX セッション（接続・Logon・Heartbeat・シーケンス番号管理）
//!
//! cTrader は価格用(QUOTE)と取引用(TRADE)の2セッションに分かれるが、
//! ntrade は価格・バー・口座情報を Open API から取得しているため、
//! ここでは取引セッションだけを張る。

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, info, warn};

use super::config::{FixConfig, TARGET_COMP_ID, TARGET_SUB_TRADE};
use super::message::{extract_message, format_timestamp, msg_type, tag, FixMessage};

/// Logon 応答を待つ上限
const LOGON_TIMEOUT: Duration = Duration::from_secs(15);

/// 受信メッセージ配信チャネルの容量
const EVENT_CHANNEL_CAPACITY: usize = 256;

trait Transport: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Transport for T {}

/// 確立済みの FIX 取引セッション
pub struct FixSession {
    config: FixConfig,
    writer: Mutex<WriteHalf<Box<dyn Transport>>>,
    /// 次に送信するメッセージのシーケンス番号
    out_seq: AtomicI64,
    /// アプリケーションメッセージの配信元（セッション管理メッセージは流さない）
    events: broadcast::Sender<FixMessage>,
    connected: AtomicBool,
}

impl FixSession {
    /// 接続して Logon を完了させる。
    ///
    /// cTrader は「セッション確立時に双方のシーケンス番号をリセットする」仕様なので、
    /// 常に MsgSeqNum=1 / ResetSeqNumFlag=Y で Logon する。
    pub async fn connect(config: FixConfig) -> Result<Arc<Self>> {
        let addr = format!("{}:{}", config.host, config.port);
        info!(
            "Connecting to cTrader FIX (host={}, port={}, tls={}, sender={})",
            config.host, config.port, config.use_tls, config.sender_comp_id
        );
        let tcp = TcpStream::connect(&addr)
            .await
            .with_context(|| format!("Failed to connect to cTrader FIX server at {addr}"))?;
        tcp.set_nodelay(true).ok();

        let stream: Box<dyn Transport> = if config.use_tls {
            let connector = native_tls::TlsConnector::new().context("Failed to build TLS connector")?;
            let connector = tokio_native_tls::TlsConnector::from(connector);
            let tls = connector
                .connect(&config.host, tcp)
                .await
                .with_context(|| format!("TLS handshake with {addr} failed"))?;
            Box::new(tls)
        } else {
            Box::new(tcp)
        };

        let (reader, writer) = tokio::io::split(stream);
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let session = Arc::new(Self {
            config,
            writer: Mutex::new(writer),
            out_seq: AtomicI64::new(1),
            events,
            connected: AtomicBool::new(true),
        });

        // Logon 応答を取りこぼさないよう、受信ループより先に購読する
        let mut logon_rx = session.events.subscribe();
        tokio::spawn(read_loop(session.clone(), reader));

        session.send_logon().await?;
        session.await_logon(&mut logon_rx).await?;

        if session.config.heartbeat_secs > 0 {
            tokio::spawn(heartbeat_loop(session.clone()));
        }
        Ok(session)
    }

    /// アプリケーションメッセージ（ExecutionReport など）の購読
    pub fn subscribe(&self) -> broadcast::Receiver<FixMessage> {
        self.events.subscribe()
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn config(&self) -> &FixConfig {
        &self.config
    }

    /// 標準ヘッダを埋めて送信する。戻り値は使用したシーケンス番号。
    pub async fn send(&self, msg: &mut FixMessage) -> Result<i64> {
        if !self.is_connected() {
            bail!("FIX session is not connected");
        }
        let seq = self.out_seq.fetch_add(1, Ordering::SeqCst);
        msg.set(tag::SENDER_COMP_ID, self.config.sender_comp_id.clone())
            .set(tag::TARGET_COMP_ID, TARGET_COMP_ID)
            .set(tag::TARGET_SUB_ID, TARGET_SUB_TRADE)
            .set(tag::MSG_SEQ_NUM, seq.to_string())
            .set(tag::SENDING_TIME, format_timestamp(Utc::now()));
        let raw = msg.encode();
        debug!("FIX → {}", msg);
        let mut w = self.writer.lock().await;
        w.write_all(&raw).await.context("Failed to write to FIX socket")?;
        w.flush().await.context("Failed to flush FIX socket")?;
        Ok(seq)
    }

    async fn send_logon(&self) -> Result<()> {
        let mut m = FixMessage::new(msg_type::LOGON);
        m.set(tag::ENCRYPT_METHOD, "0")
            .set(tag::HEART_BT_INT, self.config.heartbeat_secs.to_string())
            .set(tag::RESET_SEQ_NUM_FLAG, "Y")
            .set(tag::USERNAME, self.config.username.clone())
            .set(tag::PASSWORD, self.config.password.clone());
        self.send(&mut m).await?;
        Ok(())
    }

    async fn await_logon(&self, rx: &mut broadcast::Receiver<FixMessage>) -> Result<()> {
        let deadline = tokio::time::Instant::now() + LOGON_TIMEOUT;
        loop {
            let msg = match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Ok(m)) => m,
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                Ok(Err(broadcast::error::RecvError::Closed)) => {
                    bail!("FIX connection closed during logon")
                }
                Err(_) => bail!("FIX logon timed out after {}s", LOGON_TIMEOUT.as_secs()),
            };
            match msg.msg_type() {
                msg_type::LOGON => {
                    info!("cTrader FIX logon successful (session={})", self.config.sender_comp_id);
                    return Ok(());
                }
                // 不正な Logon には Logout が返り、理由は Text(58) に入る
                msg_type::LOGOUT => {
                    let reason = msg.get(tag::TEXT).unwrap_or("no reason given");
                    self.connected.store(false, Ordering::SeqCst);
                    bail!("cTrader rejected FIX logon: {reason}");
                }
                _ => continue,
            }
        }
    }

    /// Logout を送ってセッションを終了する
    pub async fn logout(&self) {
        if !self.is_connected() {
            return;
        }
        let mut m = FixMessage::new(msg_type::LOGOUT);
        if let Err(e) = self.send(&mut m).await {
            warn!("Failed to send FIX logout: {e:#}");
        }
        self.connected.store(false, Ordering::SeqCst);
    }

    fn mark_disconnected(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }
}

/// 受信ループ。セッション管理メッセージはここで処理し、それ以外を配信する。
async fn read_loop(session: Arc<FixSession>, mut reader: ReadHalf<Box<dyn Transport>>) {
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut chunk = [0u8; 4096];
    loop {
        let n = match reader.read(&mut chunk).await {
            Ok(0) => {
                info!("cTrader FIX connection closed by peer");
                break;
            }
            Ok(n) => n,
            Err(e) => {
                warn!("FIX socket read error: {e}");
                break;
            }
        };
        buf.extend_from_slice(&chunk[..n]);

        while let Some((raw, used)) = extract_message(&buf) {
            buf.drain(..used);
            match FixMessage::decode(&raw) {
                Ok(msg) => {
                    debug!("FIX ← {}", msg);
                    if handle_session_message(&session, &msg).await {
                        continue;
                    }
                    let _ = session.events.send(msg);
                }
                Err(e) => warn!("Failed to parse FIX message: {e}"),
            }
        }
    }
    session.mark_disconnected();
}

/// セッション管理メッセージを処理する。処理しきった場合は true（配信しない）。
///
/// Logon / Logout は接続確立の判定に使うため配信もする。
async fn handle_session_message(session: &Arc<FixSession>, msg: &FixMessage) -> bool {
    match msg.msg_type() {
        msg_type::HEARTBEAT => true,
        msg_type::TEST_REQUEST => {
            // TestRequest には同じ TestReqID を載せた Heartbeat を返す
            let mut reply = FixMessage::new(msg_type::HEARTBEAT);
            if let Some(id) = msg.get(tag::TEST_REQ_ID) {
                reply.set(tag::TEST_REQ_ID, id);
            }
            if let Err(e) = session.send(&mut reply).await {
                warn!("Failed to answer FIX test request: {e:#}");
            }
            true
        }
        msg_type::RESEND_REQUEST => {
            // 送信済みメッセージは保持していないため、GapFill で要求区間を埋める。
            // 発注は再送より「送られていないこと」が重要なので、この扱いで安全側に倒す。
            let begin = msg.get_i64(tag::BEGIN_SEQ_NO).unwrap_or(1);
            let next = session.out_seq.load(Ordering::SeqCst);
            warn!("cTrader requested resend from {begin}; replying with gap fill up to {next}");
            let mut reply = FixMessage::new(msg_type::SEQUENCE_RESET);
            reply.set(tag::GAP_FILL_FLAG, "Y").set(tag::NEW_SEQ_NO, next.to_string());
            if let Err(e) = session.send(&mut reply).await {
                warn!("Failed to send FIX sequence reset: {e:#}");
            }
            true
        }
        msg_type::SEQUENCE_RESET => true,
        msg_type::LOGOUT => {
            session.mark_disconnected();
            false
        }
        msg_type::REJECT => {
            warn!(
                "cTrader rejected FIX message (refSeqNum={:?}): {}",
                msg.get(tag::REF_SEQ_NUM),
                msg.get(tag::TEXT).unwrap_or("no reason given")
            );
            false
        }
        _ => false,
    }
}

async fn heartbeat_loop(session: Arc<FixSession>) {
    let interval = Duration::from_secs(session.config.heartbeat_secs as u64);
    loop {
        tokio::time::sleep(interval).await;
        if !session.is_connected() {
            break;
        }
        let mut m = FixMessage::new(msg_type::HEARTBEAT);
        if let Err(e) = session.send(&mut m).await {
            warn!("FIX heartbeat failed: {e:#}");
            break;
        }
    }
}

/// ExecutionReport / Reject から読み取れる拒否理由
pub fn reject_reason(msg: &FixMessage) -> String {
    msg.get(tag::TEXT)
        .map(|t| t.to_string())
        .or_else(|| msg.get(tag::ORD_REJ_REASON).map(|r| format!("OrdRejReason={r}")))
        .unwrap_or_else(|| "no reason given".to_string())
}

/// 送信前のメッセージに使う一意な ClOrdID
pub fn new_cl_ord_id(prefix: &str) -> String {
    format!("{prefix}-{}", Utc::now().timestamp_micros())
}

/// 内部エラーを anyhow に揃えるための補助
pub fn ensure_connected(session: &FixSession) -> Result<()> {
    if session.is_connected() {
        Ok(())
    } else {
        Err(anyhow!("FIX session is not connected"))
    }
}
