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
   * 4分割チャートの再生成トリガー
   */
  async generateChart(): Promise<ApiResponse<string>> {
    return await request<ApiResponse<string>>("/api/chart/generate", {
      method: "POST",
    });
  },

  /**
   * 最新チャート画像のURL
   */
  getLatestChartUrl(): string {
    return `${API_BASE_URL}/api/chart/latest?t=${Date.now()}`;
  },
};
