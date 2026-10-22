import {
  AccountMetrics,
  BacktestRunDetail,
  BacktestRunRequest,
  BacktestRunRow,
  BalancePoint,
  BarCoverage,
  BotState,
  CloseReason,
  Deployment,
  DeploymentView,
  Metrics,
  Page,
  Position,
  SignalLog,
  SignalStatus,
  StrategyInfo,
  TradeHistory,
} from "@/types/trading";

const API_BASE_URL =
  process.env.NEXT_PUBLIC_ENGINE_URL || "http://localhost:4000";

interface ApiResponse<T> {
  success: boolean;
  data?: T;
  message?: string;
}

export interface AccountInfo {
  traderLogin: number;
  ctidTraderAccountId: number;
  brokerTitle: string;
  isLive: boolean;
}

export interface OAuthUrlResponse {
  url: string;
  clientId: string;
  redirectUri: string;
}

/**
 * API共通リクエストヘルパー
 */
async function request<T>(
  path: string,
  options?: RequestInit
): Promise<T> {
  const url = `${API_BASE_URL}${path}`;
  try {
    const res = await fetch(url, {
      ...options,
      headers: {
        "Content-Type": "application/json",
        ...options?.headers,
      },
    });

    if (!res.ok) {
      const errorText = await res.text().catch(() => "");
      throw new Error(
        `API error [${res.status}]: ${errorText || res.statusText}`
      );
    }

    return await res.json();
  } catch (err: unknown) {
    console.warn(`[trading-api] Failed request to ${path}:`, err);
    throw err;
  }
}

/** 決済履歴の並び替えキー（Rust 側 TradeSort と対応） */
export type TradeSortKey =
  | "closeTime"
  | "openTime"
  | "pnlPips"
  | "pnlAmount"
  | "volumeLots"
  | "symbol";

/** 決済履歴の検索条件。未指定の項目は絞り込まない */
export interface TradeQueryParams {
  limit?: number;
  offset?: number;
  /** 銘柄・売買・決済理由・時刻・シグナルIDのフリーワード部分一致 */
  q?: string;
  signalId?: string;
  /** 管理していた戦略（デプロイ ID） */
  strategy?: string;
  symbol?: string;
  side?: "BUY" | "SELL";
  closeReason?: CloseReason;
  /** 勝ちトレードのみ / 負けトレードのみ */
  result?: "win" | "loss";
  /** 決済時刻の下限（"YYYY-MM-DD" 可） */
  from?: string;
  /** 決済時刻の上限（"YYYY-MM-DD" 指定時はその日の終わりまで） */
  to?: string;
  minPnlPips?: number;
  maxPnlPips?: number;
  sort?: TradeSortKey;
  /** true で昇順（既定は降順） */
  asc?: boolean;
}

/** 決済履歴の検索フォームの状態（入力中の値をそのまま持つので文字列） */
export interface TradeFilterState {
  q: string;
  symbol: string;
  /** デプロイ ID */
  strategy: string;
  side: "" | "BUY" | "SELL";
  closeReason: "" | CloseReason;
  result: "" | "win" | "loss";
  from: string;
  to: string;
  minPnlPips: string;
  maxPnlPips: string;
  sort: TradeSortKey;
  asc: boolean;
}

export const EMPTY_TRADE_FILTER: TradeFilterState = {
  q: "",
  symbol: "",
  strategy: "",
  side: "",
  closeReason: "",
  result: "",
  from: "",
  to: "",
  minPnlPips: "",
  maxPnlPips: "",
  sort: "closeTime",
  asc: false,
};

/** フリーワード・並び順を除く、実際に絞り込んでいる条件の数（バッジ表示用） */
export function countActiveTradeFilters(f: TradeFilterState): number {
  return [f.symbol, f.strategy, f.side, f.closeReason, f.result, f.from, f.to, f.minPnlPips, f.maxPnlPips].filter(
    (v) => v !== ""
  ).length;
}

/** フォームの状態を API のクエリに変換する（空欄・不正な数値は条件から外す） */
export function toTradeQueryParams(f: TradeFilterState): TradeQueryParams {
  const num = (v: string) => {
    const n = Number(v);
    return v.trim() !== "" && Number.isFinite(n) ? n : undefined;
  };
  return {
    q: f.q.trim() || undefined,
    symbol: f.symbol || undefined,
    strategy: f.strategy || undefined,
    side: f.side || undefined,
    closeReason: f.closeReason || undefined,
    result: f.result || undefined,
    from: f.from || undefined,
    to: f.to || undefined,
    minPnlPips: num(f.minPnlPips),
    maxPnlPips: num(f.maxPnlPips),
    sort: f.sort,
    asc: f.asc,
  };
}

/** 未指定・空・false の項目を落としてクエリ文字列にする */
function toQuery(params: object): string {
  const q = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null || value === "" || value === false) continue;
    q.set(key, String(value));
  }
  const s = q.toString();
  return s ? `?${s}` : "";
}

export const tradingApi = {
  /**
   * ボットおよび口座ステータスの取得
   */
  async getStatus(): Promise<AccountMetrics> {
    return await request<AccountMetrics>("/api/status");
  },

  /**
   * 残高の推移（古い順）。days を省略すると全期間
   */
  async getBalanceHistory(days?: number): Promise<ApiResponse<BalancePoint[]>> {
    return await request<ApiResponse<BalancePoint[]>>(`/api/balance-history${days !== undefined ? `?days=${days}` : ""}`);
  },

  /**
   * ボット稼働状態の更新 (running / paused)
   */
  async updateBotState(state: BotState): Promise<ApiResponse<BotState>> {
    return await request<ApiResponse<BotState>>("/api/control/state", {
      method: "POST",
      body: JSON.stringify({ state }),
    });
  },

  /**
   * 緊急全決済 & ボット停止
   */
  async emergencyStop(): Promise<ApiResponse<string>> {
    return await request<ApiResponse<string>>("/api/control/emergency-stop", {
      method: "POST",
    });
  },

  /**
   * 保有ポジション一覧の取得
   */
  async getPositions(): Promise<ApiResponse<Position[]>> {
    return await request<ApiResponse<Position[]>>("/api/positions");
  },

  /**
   * 単一ポジションの手動成行決済
   */
  async closePosition(id: string): Promise<ApiResponse<TradeHistory>> {
    return await request<ApiResponse<TradeHistory>>(`/api/positions/${id}/close`, {
      method: "POST",
    });
  },

  /**
   * 決済履歴の取得（既定は新しい順・ページング / 各種条件で絞り込み可）
   */
  async getTrades(params: TradeQueryParams = {}): Promise<ApiResponse<Page<TradeHistory>>> {
    return await request<ApiResponse<Page<TradeHistory>>>(`/api/trades${toQuery(params)}`);
  },

  /**
   * 決済履歴を1件削除（記録の削除のみ。ブローカー側の約定には影響しない）
   */
  async deleteTrade(id: string): Promise<ApiResponse<void>> {
    return await request<ApiResponse<void>>(`/api/trades/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  },

  /**
   * 決済履歴をまとめて削除。削除できた件数を返す
   */
  async deleteTrades(ids: string[]): Promise<ApiResponse<number>> {
    return await request<ApiResponse<number>>("/api/trades/delete", {
      method: "POST",
      body: JSON.stringify({ ids }),
    });
  },

  /**
   * cTrader Spotware OAuth 認可URLの取得
   */
  async getOAuthUrl(): Promise<ApiResponse<OAuthUrlResponse>> {
    return await request<ApiResponse<OAuthUrlResponse>>("/api/auth/ctrader/url");
  },

  /**
   * 認可コード（code）をバックエンドへ送りトークン交換 & 接続
   */
  async exchangeOAuthCode(
    code: string,
    redirectUri?: string
  ): Promise<ApiResponse<AccountInfo[]>> {
    return await request<ApiResponse<AccountInfo[]>>(
      "/api/auth/ctrader/exchange",
      {
        method: "POST",
        body: JSON.stringify({ code, redirect_uri: redirectUri }),
      }
    );
  },

  /**
   * 連携済み取引口座一覧の取得
   */
  async getAccounts(): Promise<ApiResponse<AccountInfo[]>> {
    return await request<ApiResponse<AccountInfo[]>>("/api/auth/ctrader/accounts");
  },

  /**
   * 取引口座の切り替え
   */
  async selectAccount(accountId: number): Promise<ApiResponse<string>> {
    return await request<ApiResponse<string>>(
      "/api/auth/ctrader/select-account",
      {
        method: "POST",
        body: JSON.stringify({ account_id: accountId }),
      }
    );
  },

  /** ブローカーに置いてある、まだ約定していない予約注文 */
  async getOrders(): Promise<ApiResponse<WorkingOrder[]>> {
    return await request<ApiResponse<WorkingOrder[]>>("/api/orders");
  },

  /** 予約注文をブローカーで取り消す */
  async cancelOrder(id: string): Promise<ApiResponse<string>> {
    return await request<ApiResponse<string>>(`/api/orders/${id}`, { method: "DELETE" });
  },

  /** エンジンの稼働設定（読み取り専用） */
  async getRuntime(): Promise<ApiResponse<RuntimeInfo>> {
    return await request<ApiResponse<RuntimeInfo>>("/api/runtime");
  },

  /** 事後ガード設定 */
  async getGuardConfig(): Promise<ApiResponse<GuardConfig>> {
    return await request<ApiResponse<GuardConfig>>("/api/guard/config");
  },

  /** 取引設定（対象ペア・待ち秒数・事後ガード・戦略のデプロイ） */
  async getSettings(): Promise<ApiResponse<TradingSettings>> {
    return await request<ApiResponse<TradingSettings>>("/api/settings");
  },

  /** ブローカーの銘柄一覧（資産クラス付き）。q は銘柄名・説明・資産クラスの部分一致 */
  async getSymbols(q?: string): Promise<ApiResponse<SymbolCatalogEntry[]>> {
    return await request<ApiResponse<SymbolCatalogEntry[]>>(`/api/symbols${q ? `?q=${encodeURIComponent(q)}` : ""}`);
  },

  /** 取引設定を保存（SQLite）。次の足の確定から反映。検証エラーは success:false と message */
  async saveSettings(settings: TradingSettings): Promise<ApiResponse<TradingSettings>> {
    return await request<ApiResponse<TradingSettings>>("/api/settings", {
      method: "PUT",
      body: JSON.stringify(settings),
    });
  },

  /**
   * 時間足別のチャート画像URL（cTrader の足から要求のたびに描く。建玉・予約注文の水準入り）
   */
  getLatestChartUrl(tf: ChartTimeframe, cacheKey: number = Date.now(), pair?: string): string {
    const p = pair ? `&pair=${encodeURIComponent(pair)}` : "";
    return `${API_BASE_URL}/api/chart/latest?tf=${tf}${p}&t=${cacheKey}`;
  },

  /** 使える戦略とパラメータの定義 */
  async getStrategies(): Promise<ApiResponse<StrategyInfo[]>> {
    return await request<ApiResponse<StrategyInfo[]>>("/api/strategies");
  },

  /** 設定したデプロイと、建玉・予約注文の数、直近の実行状況 */
  async getDeployments(): Promise<ApiResponse<DeploymentView[]>> {
    return await request<ApiResponse<DeploymentView[]>>("/api/deployments");
  },

  /** 戦略が出したシグナルの記録（新しい順・ページング / 条件で絞り込み可） */
  async getSignals(params: SignalQueryParams = {}): Promise<ApiResponse<Page<SignalLog>>> {
    return await request<ApiResponse<Page<SignalLog>>>(`/api/signals${toQuery(params)}`);
  },

  /** シグナル1件 */
  async getSignal(id: string): Promise<ApiResponse<SignalLog>> {
    return await request<ApiResponse<SignalLog>>(`/api/signals/${encodeURIComponent(id)}`);
  },

  /** バックテストの実行一覧（新しい順） */
  async getBacktestRuns(limit?: number): Promise<ApiResponse<BacktestRunRow[]>> {
    return await request<ApiResponse<BacktestRunRow[]>>(`/api/backtest/runs${limit !== undefined ? `?limit=${limit}` : ""}`);
  },

  /** バックテスト1件の結果（取引一覧と資産曲線付き） */
  async getBacktestRun(id: number): Promise<ApiResponse<BacktestRunDetail>> {
    return await request<ApiResponse<BacktestRunDetail>>(`/api/backtest/runs/${id}`);
  },

  /** バックテストの結果を削除 */
  async deleteBacktestRun(id: number): Promise<ApiResponse<null>> {
    return await request<ApiResponse<null>>(`/api/backtest/runs/${id}`, { method: "DELETE" });
  },

  /** 単発のバックテストを実行して保存する（終わるまで返らない。数年分の 1 分足だと 1 分ほどかかる） */
  async runBacktest(req: BacktestRunRequest): Promise<ApiResponse<{ id: number; metrics: Metrics }>> {
    return await request<ApiResponse<{ id: number; metrics: Metrics }>>("/api/backtest/run", {
      method: "POST",
      body: JSON.stringify(req),
    });
  },

  /** DB にあるヒストリカルの足の範囲 */
  async getBacktestCoverage(): Promise<ApiResponse<BarCoverage[]>> {
    return await request<ApiResponse<BarCoverage[]>>("/api/backtest/coverage");
  },
};

/** シグナル一覧の検索条件。未指定の項目は絞り込まない */
export interface SignalQueryParams {
  limit?: number;
  offset?: number;
  symbol?: string;
  /** デプロイ ID */
  deployment?: string;
  status?: SignalStatus;
  /** 理由・ガード結果・メッセージなどの部分一致 */
  q?: string;
}

/** 画面から変更できる取引設定。guard のキーは Rust 側のまま snake_case */
export interface TradingSettings {
  pairs: string[];
  barDelaySecs: number;
  guard: GuardConfig;
  /** 戦略のデプロイ（戦略は各自の時間足の確定ごとに動く） */
  deployments: Deployment[];
}

/** ブローカーの銘柄一覧の1行 */
export interface SymbolCatalogEntry {
  symbolId: number;
  symbolName: string;
  description: string | null;
  category: string | null;
  assetClass: string | null;
  baseAsset: string | null;
  quoteAsset: string | null;
  enabled: boolean;
}

export interface RuntimeInfo {
  orderMode: "paper" | "live";
  schedulerEnabled: boolean;
  pairs: string[];
  barDelaySecs: number;
  paperBalance: number;
  guardConfigPath: string;
  dbPath: string;
  ctraderTokenPresent: boolean;
  ctraderRefreshTokenPresent: boolean;
  /** Access Token の失効時刻（UTC, "YYYY-MM-DD HH:MM:SS"）。不明なら null */
  ctraderTokenExpiresAt: string | null;
  envFilePresent: boolean;
}

/** Rust 側 GuardConfig（snake_case のまま）。バックテスト（guard = "settings"）と本番の両方に当たる */
export interface GuardConfig {
  min_rr: number;
  sl_atr_min: number;
  /** 0 なら上限なし */
  sl_atr_max: number;
  /** SL 幅の上限を測る ATR の時間足（下限は常に 5M ATR） */
  sl_atr_max_tf: "5m" | "15m" | "1h" | "4h";
  sl_min_pips: number;
  max_positions_per_pair: number;
  max_positions_total: number;
  daily_loss_limit_pct: number;
  /** 予約注文の有効期限の上限（時間） */
  order_max_hours: number;
  fixed_volume_lots: number;
  risk_pct: number | null;
  pip_value_per_lot: number | null;
  /** 銘柄ごとのスプレッド上限（pips）。default は FX にだけ適用 */
  max_spread_pips: Record<string, number>;
  /** スプレッド上限（5M ATR 比、全銘柄）。null なら使わない */
  max_spread_atr: number | null;
  /** 発注後の証拠金使用率の上限（%）。null なら見ない */
  max_margin_usage_pct: number | null;
  /** 発注後の実効レバレッジの上限（倍）。null なら見ない */
  max_effective_leverage: number | null;
  news_blackout: { time: string; before_min: number; after_min: number; label: string }[];
}

/** ブローカーに置いてある予約注文（Rust 側 WorkingOrder） */
export interface WorkingOrder {
  id: string;
  symbol: string;
  side: "BUY" | "SELL";
  kind: "LIMIT" | "STOP";
  volumeLots: number;
  entryPrice: number;
  /** 0 なら未設定 */
  stopLoss: number;
  /** 0 なら未設定 */
  takeProfit: number;
  createdAt: string;
  expiresAt: string;
  /** 置いた理由など */
  note: string;
  /** この注文を置いたシグナル */
  signalId?: string;
  /** この注文を管理する戦略（デプロイ ID）。無ければ手動・取り込み */
  strategy?: string;
}

/** /api/chart/latest の tf */
export type ChartTimeframe = "1M" | "5M" | "15M" | "1H" | "4H" | "1D";
export const CHART_TIMEFRAMES: ChartTimeframe[] = ["1D", "4H", "1H", "15M", "5M", "1M"];
