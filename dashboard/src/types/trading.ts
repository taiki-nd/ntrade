export type BotState = "running" | "paused" | "circuit_breaker";

export interface Position {
  id: string;
  symbol: "USDJPY" | "EURUSD" | string;
  side: "BUY" | "SELL";
  volumeLots: number;
  entryPrice: number;
  currentPrice: number;
  /** 0 なら未設定 */
  stopLoss: number;
  /** 0 なら未設定 */
  takeProfit: number;
  pnlPips: number;
  pnlAmount: number;
  openTime: string;
  /** 建てた理由など（戦略のシグナル理由・ブローカーから取り込んだ旨） */
  note: string;
  /** この建玉を建てたシグナル */
  signalId?: string;
  /** この建玉を管理する戦略（デプロイ ID）。無ければ手動・取り込み */
  strategy?: string;
}

/**
 * 決済理由。STRATEGY は戦略の決済シグナルによる決済。
 * INVALIDATED / DISCRETIONARY は LLM 運用時代の旧データにだけ残っている。
 */
export type CloseReason =
  | "TAKE_PROFIT"
  | "STOP_LOSS"
  | "STRATEGY"
  | "MANUAL"
  | "CIRCUIT_BREAKER"
  | "INVALIDATED"
  | "DISCRETIONARY";

export interface TradeHistory {
  id: string;
  symbol: string;
  side: "BUY" | "SELL";
  volumeLots: number;
  entryPrice: number;
  closePrice: number;
  stopLoss: number;
  takeProfit: number;
  pnlPips: number;
  pnlAmount: number;
  closeReason: CloseReason;
  openTime: string;
  closeTime: string;
  /** エントリーしたシグナル */
  signalId?: string;
  /** 管理していた戦略（デプロイ ID）。無ければ手動・取り込み */
  strategy?: string;
}

/** 残高の推移の1点（決済・入出金の直後の残高。time はエンジンの UTC 文字列） */
export interface BalancePoint {
  time: string;
  balance: number;
}

export interface AccountMetrics {
  botState: BotState;
  balance: number;
  equity: number;
  margin: number;
  freeMargin: number;
  /** 今日（UTC）決済した取引の損益合計 */
  dailyPnl: number;
  dailyPnlPercent: number;
  unrealizedPnl: number;
  winRateToday: number;
  totalTradesToday: number;
  winningTradesToday: number;
  /** ペアごとの直近スプレッド（pips、気配値から） */
  spreads: Record<string, number>;
  circuitBreakerThresholdPercent: number;
  /** "paper" | "live"。live のとき balance はブローカー残高 */
  orderMode: "paper" | "live";
  /** cTrader から取得したブローカー口座残高（未取得なら undefined） */
  brokerBalance?: number;
  connectionStatus: {
    ctrader: "connected" | "connecting" | "disconnected";
    pingMs: number;
    environment: "DEMO" | "LIVE";
    accountNumber: string;
  };
}

/** 一覧 API のページング結果 */
export interface Page<T> {
  items: T[];
  /** 条件に一致する全件数 */
  total: number;
}

// ---------------------------------------------------------------------------
// 戦略・デプロイ
// ---------------------------------------------------------------------------

/** 戦略を動かす時間足（Rust 側 BarPeriod のシリアライズ名） */
export type StrategyTimeframe = "M1" | "M5" | "M15" | "M30" | "H1" | "H4" | "D1";
export const STRATEGY_TIMEFRAMES: StrategyTimeframe[] = ["M1", "M5", "M15", "M30", "H1", "H4", "D1"];

/** どの戦略を・どの時間足で・どのパラメータで動かすか */
export interface StrategySpec {
  name: string;
  timeframe: StrategyTimeframe;
  /** 省略したパラメータは戦略の既定値 */
  params: Record<string, number>;
}

export interface StrategyParamInfo {
  name: string;
  default: number;
  min: number;
  max: number;
  /** 整数で使うパラメータか（期間など） */
  integer: boolean;
  description: string;
}

/** GET /api/strategies の1件 */
export interface StrategyInfo {
  name: string;
  title: string;
  description: string;
  params: StrategyParamInfo[];
}

/** 戦略を 1 銘柄で動かす設定（取引設定の deployments） */
export interface Deployment {
  /** 英数字と - _（48 文字まで）。注文の comment と取引履歴の strategy 列に入る */
  id: string;
  /** 取引設定の pairs にある銘柄 */
  pair: string;
  strategy: StrategySpec;
  enabled: boolean;
  /** true なら発注せず、シグナルを記録するだけ */
  dryRun: boolean;
}

/** デプロイごとの直近の実行状況（エンジンの再起動で消える） */
export interface DeploymentStatus {
  id: string;
  lastRun: string | null;
  /** 直近の判断に使った足の確定時刻 */
  lastBar: string | null;
  /** 直近の判断で出た操作の数 */
  lastActions: number;
  lastError: string | null;
}

/** GET /api/deployments の1件 */
export interface DeploymentView extends Deployment {
  openPositions: number;
  workingOrders: number;
  status: DeploymentStatus | null;
}

// ---------------------------------------------------------------------------
// シグナル
// ---------------------------------------------------------------------------

export type SignalAction = "OPEN" | "CLOSE" | "CANCEL" | "MODIFY";
export type SignalStatus = "EXECUTED" | "REJECTED" | "FAILED" | "DRY_RUN";

/** 戦略が出した操作の記録 */
export interface SignalLog {
  id: string;
  timestamp: string;
  /** デプロイ ID */
  deployment: string;
  /** 表示用ラベル（"ema_cross 5M fast=20 slow=50"） */
  strategy: string;
  symbol: string;
  action: SignalAction;
  side: string | null;
  reason: string;
  /** 操作の中身と、その時点の価格・スプレッド・ATR・ロットなど */
  detail: unknown;
  /** "PASS" か、通らなかったチェック名の "|" 区切り */
  guardResult: string | null;
  status: SignalStatus;
  message: string | null;
  orderId: string | null;
  dryRun: boolean;
}

// ---------------------------------------------------------------------------
// バックテスト
// ---------------------------------------------------------------------------

export interface MonthlyMetrics {
  /** "YYYY-MM" */
  month: string;
  trades: number;
  net: number;
  pips: number;
}

/** バックテストの成績（金額は口座通貨、*Pct / winRate は %） */
export interface Metrics {
  initialBalance: number;
  finalBalance: number;
  trades: number;
  wins: number;
  losses: number;
  winRate: number;
  netProfit: number;
  grossProfit: number;
  grossLoss: number;
  profitFactor: number | null;
  expectancy: number;
  avgWin: number;
  avgLoss: number;
  payoffRatio: number | null;
  totalPips: number;
  avgPips: number;
  maxDrawdown: number;
  maxDrawdownPct: number;
  returnPct: number;
  sharpe: number | null;
  recoveryFactor: number | null;
  maxConsecutiveWins: number;
  maxConsecutiveLosses: number;
  avgHoldMinutes: number;
  commission: number;
  swap: number;
  longTrades: number;
  longWinRate: number;
  shortTrades: number;
  shortWinRate: number;
  exitReasons: Record<string, number>;
  monthly: MonthlyMetrics[];
}

export type BacktestKind = "single" | "sweep" | "walk_forward";

/** グリッドサーチの1組（最良が先頭） */
export interface SweepRow {
  params: Record<string, number>;
  metrics: Metrics;
  score: number | null;
}

/** ウォークフォワードの1窓 */
export interface WalkForwardWindow {
  trainFrom: string;
  trainTo: string;
  testFrom: string;
  testTo: string;
  /** 学習期間で選んだパラメータ（選べなければ null） */
  params: Record<string, number> | null;
  train: Metrics | null;
  test: Metrics | null;
}

/** 単発実行の config（実行条件と、シグナルの集計） */
export interface BacktestSingleConfig {
  config?: {
    pair: string;
    from: string;
    to: string;
    strategy: StrategySpec;
    guard: unknown;
    costs: {
      spread_pips: number;
      slippage_pips: number;
      commission_per_lot: number;
      swap_long_per_lot: number;
      swap_short_per_lot: number;
      rollover_hour_utc: number;
    };
    initialBalance: number;
    accountCurrency: string;
  };
  execPeriod?: string;
  conversion?: unknown;
  decisions?: number;
  signals?: {
    intents: number;
    placed: number;
    /** ガードで弾かれたチェック名ごとの件数 */
    rejected: Record<string, number>;
    rejectedTotal: number;
    closes: number;
    cancels: number;
    modifies: number;
  };
}

export interface BacktestRunRow {
  id: number;
  createdAt: string;
  label: string;
  kind: BacktestKind;
  pair: string;
  /** 戦略のラベル */
  strategy: string;
  fromTs: string;
  toTs: string;
  /** single なら BacktestSingleConfig。ほかは実行時の条件（形は種類による） */
  config: unknown;
  /** sweep は最良の組の成績、walk_forward はテスト期間をつないだ成績 */
  metrics: Metrics;
  /** sweep は SweepRow[]、walk_forward は WalkForwardWindow[]、single は null */
  summary: unknown;
}

export interface BacktestTrade {
  id: number | string;
  side: "BUY" | "SELL";
  volumeLots: number;
  entryKind: "MARKET" | "LIMIT" | "STOP";
  entryTime: string;
  entryPrice: number;
  exitTime: string;
  exitPrice: number;
  stopLoss: number | null;
  takeProfit: number | null;
  pnlPips: number;
  gross: number;
  commission: number;
  swap: number;
  net: number;
  exitReason: "TAKE_PROFIT" | "STOP_LOSS" | "STRATEGY" | "END_OF_TEST" | string;
  entryReason: string;
  exitNote: string;
  maePips: number;
  mfePips: number;
}

/** 資産曲線の1点（time は RFC3339 UTC） */
export interface EquityPoint {
  time: string;
  balance: number;
  equity: number;
}

export interface BacktestRunDetail {
  run: BacktestRunRow;
  trades: BacktestTrade[];
  /** 間引き済み（2000 点程度まで）。sweep は空 */
  equity: EquityPoint[];
}

/** POST /api/backtest/run の本文。省略した項目は config/backtest.toml の値 */
export interface BacktestRunRequest {
  pair: string;
  /** RFC3339 */
  from: string;
  /** RFC3339 */
  to: string;
  strategy: StrategySpec;
  initialBalance?: number;
  spreadPips?: number;
  commissionPerLot?: number;
  execTimeframe?: StrategyTimeframe;
  label: string;
}

/** グリッドサーチ・ウォークフォワードで何を最大化するか */
export type Objective = "net_profit" | "profit_factor" | "sharpe" | "recovery_factor" | "expectancy";

/** グリッドサーチの本文。grid は "fast=10..30:5" / "slow=50,100" の形 */
export interface GridRequest extends BacktestRunRequest {
  grid: string[];
  objective: Objective;
  minTrades: number;
}

/** POST /api/backtest/jobs の本文 */
export type BacktestJobRequest =
  | ({ kind: "fetch" } & { pairs: string[]; from: string; to?: string; periods?: StrategyTimeframe[] })
  | ({ kind: "sweep" } & GridRequest)
  | ({ kind: "walk_forward" } & GridRequest & { trainDays: number; testDays: number });

export type BacktestJobKind = "fetch" | "sweep" | "walk_forward";
export type BacktestJobStatus = "queued" | "running" | "succeeded" | "failed" | "cancelled";

/** エンジンの中で 1 件ずつ走るジョブ（再起動で消える） */
export interface BacktestJob {
  id: number;
  kind: BacktestJobKind;
  title: string;
  status: BacktestJobStatus;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  /** 取得なら区切りの数、探索ならバックテストの回数 */
  done: number;
  total: number;
  message: string | null;
  /** 保存したバックテスト結果（グリッドサーチ・ウォークフォワード） */
  runId: number | null;
  error: string | null;
}

/** DB にある足の範囲（period は "1M" / "5M" / ... / "1D"） */
export interface BarCoverage {
  pair: string;
  period: string;
  count: number;
  first: string | null;
  last: string | null;
}
