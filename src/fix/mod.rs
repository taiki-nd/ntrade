//! cTrader FIX API クライアント（発注経路）
//!
//! ブローカーによっては Open API 経由の取引が無効化されており
//! （新規注文に `TRADING_DISABLED` が返る）、その場合の発注手段が FIX API になる。
//!
//! FIX API には「ヒストリカルデータを取得できない」「口座情報（残高・レバレッジ・
//! 証拠金）を取得できない」という公式の制限があるため、ntrade では
//! **データ取得・口座情報・ポジション照合は Open API のまま、発注だけを FIX** に
//! 委ねるハイブリッド構成を取る。

pub mod config;
pub mod message;
pub mod session;
pub mod trading;

pub use config::FixConfig;
pub use message::FixMessage;
pub use session::FixSession;
pub use trading::{lots_to_units, FixFill, FixTradingClient};
