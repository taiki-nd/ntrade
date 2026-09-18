export type BotState = "running" | "paused" | "circuit_breaker";

export interface Position {
  id: string;
  symbol: "USDJPY" | "EURUSD" | string;
  side: "BUY" | "SELL";
  volumeLots: number;
  entryPrice: number;
  currentPrice: number;
  stopLoss: number;
  takeProfit: number;
  pnlPips: number;
  pnlAmount: number;
  openTime: string;
  invalidationReason: string;
  /** このポジションを建てた判断（CoT ログ ID） */
  cotLogId?: string;
}

export type CloseReason = "TAKE_PROFIT" | "STOP_LOSS" | "MANUAL" | "CIRCUIT_BREAKER";

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
  cotLogId?: string;
}

export type ActionType = "BUY" | "SELL" | "HOLD";
export type TriggerPatternType = "PINBAR" | "ENGULFING" | "FAKEOUT" | "NONE";

export interface CoTLog {
  id: string;
  timestamp: string;
  symbol: string;
  action: ActionType;
  confidence: number;
  entryType?: "MARKET" | "LIMIT" | "CONDITIONAL";
  entryPrice?: number;
  stopLoss?: number;
  takeProfit?: number;
  riskRewardRatio?: number;
  /** 環境認識（4H/1H） */
  macroContext: string;
  /** 注文の攻防（15M/5M） */
  orderFlow: string;
  /** 無効化ライン（SLの根拠） */
  invalidation: string;
  /** 反対材料 */
  conflicts?: string;
  /** ガードで弾かれた場合のガード名 */
  guardResult?: string;
  reasoning: string;
  executed: boolean;
  spreadPips: number;
}

export interface AccountMetrics {
  botState: BotState;
  balance: number;
  equity: number;
  margin: number;
  freeMargin: number;
  dailyPnl: number;
  dailyPnlPercent: number;
  unrealizedPnl: number;
  winRateToday: number;
  totalTradesToday: number;
  winningTradesToday: number;
  usdjpySpread: number;
  eurusdSpread: number;
  circuitBreakerThresholdPercent: number;
  /** "paper" | "live"。live のとき balance はブローカー残高 */
  orderMode: "paper" | "live";
  /** cTrader から取得したブローカー口座残高（未取得なら undefined） */
  brokerBalance?: number;
  connectionStatus: {
    ctrader: "connected" | "connecting" | "disconnected";
    llm: "ready" | "busy" | "error";
    pingMs: number;
    environment: "DEMO" | "LIVE";
    accountNumber: string;
  };
}

export interface LessonLearned {
  id: string;
  createdAt: string;
  symbol: string;
  rule: string;
  context: string;
  active: boolean;
  triggerTradeId?: string;
  category: "RISK" | "TIMING" | "PATTERN" | "NEWS";
}

/** 一覧 API のページング結果 */
export interface Page<T> {
  items: T[];
  /** 条件に一致する全件数 */
  total: number;
}

/** 判断1件と、そこから生まれた取引 */
export interface CoTDetail {
  log: CoTLog;
  /** 決済済みの取引 */
  trades: TradeHistory[];
  /** 保有中のポジション */
  openPositions: Position[];
}
