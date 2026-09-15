import {
  AccountMetrics,
  BotState,
  CoTLog,
  LessonLearned,
  Position,
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

export const tradingApi = {
  /**
   * ボットおよび口座ステータスの取得
   */
  async getStatus(): Promise<AccountMetrics> {
    return await request<AccountMetrics>("/api/status");
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
   * 約定・決済履歴一覧の取得
   */
  async getTrades(): Promise<ApiResponse<TradeHistory[]>> {
    return await request<ApiResponse<TradeHistory[]>>("/api/trades");
  },

  /**
   * LLM思考プロセス（CoT）ログ一覧の取得
   */
  async getCoTLogs(): Promise<ApiResponse<CoTLog[]>> {
    return await request<ApiResponse<CoTLog[]>>("/api/cot");
  },

  /**
   * 教訓ルール一覧の取得
   */
  async getLessons(): Promise<ApiResponse<LessonLearned[]>> {
    return await request<ApiResponse<LessonLearned[]>>("/api/lessons");
  },

  /**
   * 新規教訓ルールの登録
   */
  async createLesson(
    lesson: Omit<LessonLearned, "id" | "createdAt">
  ): Promise<ApiResponse<LessonLearned>> {
    return await request<ApiResponse<LessonLearned>>("/api/lessons", {
      method: "POST",
      body: JSON.stringify(lesson),
    });
  },

  /**
   * 教訓ルールの有効/無効切り替え
   */
  async toggleLesson(id: string): Promise<ApiResponse<LessonLearned>> {
    return await request<ApiResponse<LessonLearned>>(`/api/lessons/${id}/toggle`, {
      method: "POST",
    });
  },

  /**
   * 教訓ルールの削除
   */
  async deleteLesson(id: string): Promise<ApiResponse<void>> {
    return await request<ApiResponse<void>>(`/api/lessons/${id}`, {
      method: "DELETE",
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

  /**
   * Snapshot（時間足別チャート4枚 + 客観的事実JSON）の再生成トリガー
   */
  async generateChart(): Promise<ApiResponse<string>> {
    return await request<ApiResponse<string>>("/api/chart/generate", {
      method: "POST",
    });
  },

  /**
   * 時間足別の最新チャート画像URL
   */
  getLatestChartUrl(tf: ChartTimeframe, cacheKey: number = Date.now()): string {
    return `${API_BASE_URL}/api/chart/latest?tf=${tf}&t=${cacheKey}`;
  },

  /**
   * 直近 Snapshot の客観的事実JSON
   */
  async getLatestSnapshot(): Promise<ApiResponse<MarketSnapshot>> {
    return await request<ApiResponse<MarketSnapshot>>("/api/snapshot/latest");
  },
};

export type ChartTimeframe = "4H" | "1H" | "15M" | "5M";
export const CHART_TIMEFRAMES: ChartTimeframe[] = ["4H", "1H", "15M", "5M"];

/** Rust 側 MarketSnapshot の表示に必要な部分だけを型付け */
export interface MarketSnapshot {
  pair: string;
  timestamp: string;
  session: string;
  current_price: number;
  spread_pips: number;
  volatility: {
    atr14_5m_pips: number | null;
    atr14_15m_pips: number | null;
    atr14_1h_pips: number | null;
    atr14_4h_pips: number | null;
    today_range_pips: number | null;
    avg_daily_range_pips: number | null;
    avg_daily_range_days_sampled: number;
    today_range_vs_avg: number | null;
  };
  reference_levels: {
    prev_day_high: number | null;
    prev_day_low: number | null;
    today_high: number | null;
    today_low: number | null;
    asia_session_high: number | null;
    asia_session_low: number | null;
    round_numbers: number[];
  };
  latest_bars: {
    "4h": string | null;
    "1h": string | null;
    "15m": string | null;
    "5m": string | null;
  };
}
