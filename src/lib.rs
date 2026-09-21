pub mod chart;
pub mod ctrader;
pub mod executor;
pub mod fix;
pub mod guard;
pub mod llm;
pub mod reflection;
pub mod replay;
pub mod scheduler;
pub mod server;
pub mod snapshot;
pub mod storage;
pub mod strategy;

pub use chart::{ChartPlotter, ChartPlotterConfig, PriceLevel, TimeframeChart};
pub use ctrader::{BarPeriod, CandleBar, CTraderConfig, CTraderService, SymbolInfo};
pub use llm::{LlmBackend, LlmClient, LlmClientConfig};
pub use server::{create_router, AppState};
pub use snapshot::{
    AccountState, ChartSet, MarketSnapshot, SnapshotBundle, SnapshotConfig, SnapshotInput,
    SnapshotPipeline,
};
pub use strategy::{PromptBuilder, TradeDecision};
