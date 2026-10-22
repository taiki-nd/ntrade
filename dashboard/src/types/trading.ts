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
  /** 終値判定モードの損切りライン（LLM の SL）。このとき stopLoss はブローカーに置いたハードSL */
  closeStop?: number;
}

export type CloseReason = "TAKE_PROFIT" | "STOP_LOSS" | "INVALIDATED" | "DISCRETIONARY" | "MANUAL" | "CIRCUIT_BREAKER";

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
  /** 新規注文の種類。MARKET / CONDITIONAL は旧データ */
  entryType?: "LIMIT" | "STOP" | "MARKET" | "CONDITIONAL";
  entryPrice?: number;
  stopLoss?: number;
  takeProfit?: number;
  riskRewardRatio?: number;
  /** ファンダメンタルズの読み（事実から読んだ地合い・根拠・イベントリスク）。旧データは空 */
  fundamentalView?: string;
  /** 流れの読み（上位足と直近の値動き）。旧データは 4H/1H の環境認識・価格の位置 */
  macroContext: string;
  /** 注文の価格・利確の根拠。旧データは 15M/5M の注文の攻防 */
  orderFlow: string;
  /** 無効化ライン（SLの根拠） */
  invalidation: string;
  /** 反対材料 */
  conflicts?: string;
  /** 新規注文のガード不通過・発注エラーと、既存の注文・建玉への操作の結果（`splitGuardResult` で分ける） */
  guardResult?: string;
  reasoning: string;
  executed: boolean;
  spreadPips: number;
  /**
   * 条件付きプランの成立ログの場合、そのプランを立てた LLM 判断の ID。
   * プランは HOLD 判断と一緒に出るため、発注した記録と根拠の判断は別ログになる。
   */
  originCotLogId?: string;
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
  dailyPnl: number;
  dailyPnlPercent: number;
  unrealizedPnl: number;
  winRateToday: number;
  totalTradesToday: number;
  winningTradesToday: number;
  /** ペアごとの直近スプレッド（pips） */
  spreads: Record<string, number>;
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
  /** プラン成立を受けた再判断（旧データは成立ログ）の場合、そのプランを立てた LLM 判断 */
  origin?: CoTLog;
}
