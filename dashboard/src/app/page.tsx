"use client";

import * as React from "react";
import { DashboardLayout } from "@/components/layouts/dashboard-layout";
import { BotControlHeader } from "@/components/trading/BotControlHeader";
import { PositionTable } from "@/components/trading/PositionTable";
import { ChartPreview } from "@/components/trading/ChartPreview";
import { TradeHistoryTable } from "@/components/trading/TradeHistoryTable";
import { OrderMonitor } from "@/components/trading/OrderMonitor";
import { BalanceChart } from "@/components/trading/BalanceChart";
import { DeploymentStatusCard } from "@/components/trading/DeploymentStatusCard";
import { SignalViewer } from "@/components/trading/SignalViewer";
import { BacktestPanel } from "@/components/trading/BacktestPanel";
import { Tabs, TabsContent } from "@/components/ui/tabs";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import {
  BotState,
  AccountMetrics,
  DeploymentView,
  Position,
  TradeHistory,
  Page,
} from "@/types/trading";
import {
  EMPTY_TRADE_FILTER,
  TradeFilterState,
  toTradeQueryParams,
  tradingApi,
} from "@/lib/trading-api";
import { useHash } from "@/hooks/use-hash";
import { toast } from "sonner";
import { PlugZap } from "lucide-react";

type EngineStatus = "connecting" | "online" | "offline";

const TAB_IDS = ["overview", "signals", "chart", "trades", "backtest"];

const TRADE_PAGE_SIZE = 20;

const emptyPage = <T,>(): Page<T> => ({ items: [], total: 0 });

/** 常時更新が要るもの（口座状況・保有ポジション）の取得間隔 */
const LIVE_INTERVAL = 5000;
/** 表示中のタブでしか使わない一覧系の取得間隔 */
const LIST_INTERVAL = 30000;

const errorMessage = (err: unknown) =>
  err instanceof Error ? err.message : "エンジンが起動しているか確認してください。";

/**
 * `enabled` の間だけ `fetcher` を即時実行し、以降 `intervalMs` ごとに繰り返す。
 * `fetcher` の同一性が変われば（ページ送り・検索条件の変更）その場で取得し直す。
 */
function usePolling(fetcher: () => void, intervalMs: number, enabled: boolean) {
  React.useEffect(() => {
    if (!enabled) return;
    fetcher();
    const interval = setInterval(fetcher, intervalMs);
    return () => clearInterval(interval);
  }, [fetcher, intervalMs, enabled]);
}

export default function TradingDashboard() {
  const [engineStatus, setEngineStatus] = React.useState<EngineStatus>("connecting");
  const [metrics, setMetrics] = React.useState<AccountMetrics | null>(null);
  const [positions, setPositions] = React.useState<Position[]>([]);
  // 戦略のデプロイと直近の実行状況（null の間は読み込み中）
  const [deployments, setDeployments] = React.useState<DeploymentView[] | null>(null);
  const [tradePage, setTradePage] = React.useState(1);
  const [tradesPage, setTradesPage] = React.useState<Page<TradeHistory>>(emptyPage);
  // 取引履歴の検索条件（入力中は打鍵ごとに取得しないよう遅延させる）
  const [tradeFilter, setTradeFilter] = React.useState<TradeFilterState>(EMPTY_TRADE_FILTER);
  const [tradeFilterDebounced, setTradeFilterDebounced] = React.useState<TradeFilterState>(EMPTY_TRADE_FILTER);
  // 銘柄プルダウンの選択肢（エンジンの稼働設定から取得）
  const [pairs, setPairs] = React.useState<string[]>([]);
  // 決済・取消の直後に予約注文の一覧を取り直させる
  const [ordersKey, setOrdersKey] = React.useState(0);
  // 表示するビューは URL ハッシュで決まる（サイドバーの `/#signals` などから切り替える）
  const hash = useHash();
  const activeTab = TAB_IDS.includes(hash) ? hash : "overview";

  // 口座状況と保有ポジションだけは常時最新に保つ（エンジン疎通の判定もここで行う）
  const fetchLive = React.useCallback(async () => {
    try {
      const [m, pRes] = await Promise.all([tradingApi.getStatus(), tradingApi.getPositions()]);
      setMetrics(m);
      if (pRes.success && pRes.data) setPositions(pRes.data);
      setEngineStatus("online");
    } catch {
      setEngineStatus("offline");
    }
  }, []);

  // デプロイは概要のカードと、シグナル・取引履歴の絞り込みの選択肢に使う
  const fetchDeployments = React.useCallback(async () => {
    try {
      const res = await tradingApi.getDeployments();
      if (res.success && res.data) setDeployments(res.data);
    } catch {
      // 疎通エラーは fetchLive 側で表示するのでここでは握りつぶす
    }
  }, []);

  // 取引履歴タブの一覧（ページ・絞り込み条件が変われば即時取得し直す）
  const fetchTrades = React.useCallback(async () => {
    try {
      const res = await tradingApi.getTrades({
        ...toTradeQueryParams(tradeFilterDebounced),
        limit: TRADE_PAGE_SIZE,
        offset: (tradePage - 1) * TRADE_PAGE_SIZE,
      });
      if (res.success && res.data) setTradesPage(res.data);
    } catch {
      // 同上
    }
  }, [tradePage, tradeFilterDebounced]);

  // 手動リフレッシュや決済の直後に、常時更新分と表示中タブの分だけを取り直す
  const refresh = React.useCallback(async () => {
    const byTab: Record<string, () => Promise<void>> = {
      trades: fetchTrades,
    };
    setOrdersKey((k) => k + 1);
    await Promise.all([fetchLive(), fetchDeployments(), byTab[activeTab]?.()]);
  }, [activeTab, fetchLive, fetchDeployments, fetchTrades]);

  // 入力が落ち着いてから検索を実行する
  React.useEffect(() => {
    const t = setTimeout(() => setTradeFilterDebounced(tradeFilter), 300);
    return () => clearTimeout(t);
  }, [tradeFilter]);

  // 銘柄の選択肢は起動後に一度だけ取得する
  React.useEffect(() => {
    tradingApi
      .getRuntime()
      .then((res) => {
        if (res.success && res.data) setPairs(res.data.pairs);
      })
      .catch(() => undefined);
  }, []);

  // タブに関係なく常時ポーリングするのはこれだけ
  usePolling(fetchLive, LIVE_INTERVAL, true);
  // デプロイは複数のタブで使うので、ゆるめの間隔で常に取る
  usePolling(fetchDeployments, LIST_INTERVAL, true);
  // 一覧系は表示中のタブの分だけを、ゆるめの間隔で更新する
  usePolling(fetchTrades, LIST_INTERVAL, activeTab === "trades");

  const deploymentIds = React.useMemo(() => (deployments ?? []).map((d) => d.id), [deployments]);

  // ボット稼働トグル切り替え
  const handleStateChange = async (newState: BotState) => {
    try {
      const res = await tradingApi.updateBotState(newState);
      if (!res.success) {
        toast.error("ボット状態の更新に失敗しました", { description: res.message });
        return;
      }
      if (newState === "running") {
        toast.success("自動売買エンジンの稼働を開始しました", {
          description: "有効なデプロイが、それぞれの時間足の確定ごとに戦略を実行します。",
        });
      } else {
        toast.warning("自動売買を一時停止しました", {
          description: "戦略の実行と新規発注を止めます。置いてある予約注文はブローカーに残り、約定しうるので、不要なら取り消してください。",
        });
      }
    } catch (err) {
      toast.error("ボット状態の更新に失敗しました", { description: errorMessage(err) });
    } finally {
      refresh();
    }
  };

  // 緊急全決済 & 停止
  const handleEmergencyStop = async () => {
    try {
      const res = await tradingApi.emergencyStop();
      toast.error("緊急全決済を執行しました", {
        description: res.data || res.message || "予約注文を取り消し、保有中のポジションを成行決済して、自動売買を停止しました。",
      });
    } catch (err) {
      toast.error("緊急全決済の送信に失敗しました", { description: errorMessage(err) });
    } finally {
      refresh();
    }
  };

  // 単一ポジションの手動決済
  const handleClosePosition = async (id: string) => {
    try {
      const res = await tradingApi.closePosition(id);
      if (res.success && res.data) {
        toast.success(`${res.data.symbol} ポジションを成行決済しました`, {
          description: `損益: ${res.data.pnlPips >= 0 ? "+" : ""}${res.data.pnlPips.toFixed(1)} pips (¥${res.data.pnlAmount.toLocaleString()})`,
        });
      } else {
        toast.error("ポジションの決済に失敗しました", { description: res.message });
      }
    } catch (err) {
      toast.error("ポジションの決済に失敗しました", { description: errorMessage(err) });
    } finally {
      refresh();
    }
  };

  // 取引履歴の削除（1件でも複数でも同じ導線）
  const handleDeleteTrades = async (ids: string[]) => {
    try {
      const res = ids.length === 1 ? await tradingApi.deleteTrade(ids[0]) : await tradingApi.deleteTrades(ids);
      if (res.success) {
        toast.info(res.message || `取引履歴を ${ids.length} 件削除しました`);
      } else {
        toast.error("取引履歴の削除に失敗しました", { description: res.message });
      }
    } catch (err) {
      toast.error("取引履歴の削除に失敗しました", { description: errorMessage(err) });
    } finally {
      refresh();
    }
  };

  return (
    <DashboardLayout
      title="ntrade 自動売買ダッシュボード"
      headerContent={
        // 稼働ステータス・口座サマリー・緊急停止をヘッダーに常時表示
        metrics ? (
          <BotControlHeader
            botState={metrics.botState}
            metrics={metrics}
            onStateChange={handleStateChange}
            onEmergencyStop={handleEmergencyStop}
            onRefresh={refresh}
          />
        ) : (
          <Skeleton className="h-9 flex-1 rounded-lg" />
        )
      }
    >
      <div className="space-y-6">
        {engineStatus === "offline" && (
          <Alert variant="destructive">
            <PlugZap className="h-4 w-4" />
            <AlertTitle>Rustコアエンジンに接続できません</AlertTitle>
            <AlertDescription>
              {`${process.env.NEXT_PUBLIC_ENGINE_URL || "http://localhost:4000"} への接続に失敗しました。エンジンを起動すると5秒以内に自動で再接続します。`}
            </AlertDescription>
          </Alert>
        )}

        {/* メインビュータブ */}
        <Tabs value={activeTab} className="space-y-4">
          {/* 全体概要タブ */}
          <TabsContent value="overview" className="space-y-4">
            {/* 残高の推移、稼働中の戦略と、口座の中身（建玉・予約注文） */}
            <div className="space-y-6">
              <BalanceChart refreshKey={ordersKey} />
              <DeploymentStatusCard deployments={deployments} />
              <PositionTable positions={positions} onClosePosition={handleClosePosition} />
              <OrderMonitor refreshKey={ordersKey} onChanged={refresh} />
            </div>
          </TabsContent>

          {/* シグナル タブ */}
          <TabsContent value="signals" className="space-y-4">
            <SignalViewer pairs={pairs} deployments={deploymentIds} />
          </TabsContent>

          {/* チャート タブ */}
          <TabsContent value="chart" className="space-y-4">
            <ChartPreview />
          </TabsContent>

          {/* 取引履歴 タブ */}
          <TabsContent value="trades" className="space-y-4">
            <TradeHistoryTable
              trades={tradesPage.items}
              pagination={{
                page: tradePage,
                pageSize: TRADE_PAGE_SIZE,
                total: tradesPage.total,
                onPageChange: setTradePage,
              }}
              filter={{
                value: tradeFilter,
                symbols: pairs,
                strategies: deploymentIds,
                onChange: (value) => {
                  setTradeFilter(value);
                  setTradePage(1);
                },
              }}
              onDelete={handleDeleteTrades}
            />
          </TabsContent>

          {/* バックテスト タブ */}
          <TabsContent value="backtest" className="space-y-4">
            <BacktestPanel pairs={pairs} />
          </TabsContent>
        </Tabs>
      </div>
    </DashboardLayout>
  );
}
