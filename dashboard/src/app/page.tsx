"use client";

import * as React from "react";
import { DashboardLayout } from "@/components/layouts/dashboard-layout";
import { BotControlHeader } from "@/components/trading/BotControlHeader";
import { PositionTable } from "@/components/trading/PositionTable";
import { CoTViewer } from "@/components/trading/CoTViewer";
import { ChartPreview } from "@/components/trading/ChartPreview";
import { TradeHistoryTable } from "@/components/trading/TradeHistoryTable";
import { LessonsManager } from "@/components/trading/LessonsManager";
import { ReplayViewer } from "@/components/trading/ReplayViewer";
import { PlanMonitor } from "@/components/trading/PlanMonitor";
import { Tabs, TabsContent } from "@/components/ui/tabs";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import {
  BotState,
  AccountMetrics,
  Position,
  TradeHistory,
  CoTLog,
  LessonLearned,
} from "@/types/trading";
import { tradingApi } from "@/lib/trading-api";
import { useHash } from "@/hooks/use-hash";
import { toast } from "sonner";
import { PlugZap } from "lucide-react";

type EngineStatus = "connecting" | "online" | "offline";

const TAB_IDS = ["overview", "cot", "chart", "trades", "lessons", "replay"];

const errorMessage = (err: unknown) =>
  err instanceof Error ? err.message : "エンジンが起動しているか確認してください。";

export default function TradingDashboard() {
  const [engineStatus, setEngineStatus] = React.useState<EngineStatus>("connecting");
  const [metrics, setMetrics] = React.useState<AccountMetrics | null>(null);
  const [positions, setPositions] = React.useState<Position[]>([]);
  const [trades, setTrades] = React.useState<TradeHistory[]>([]);
  const [cotLogs, setCotLogs] = React.useState<CoTLog[]>([]);
  const [lessons, setLessons] = React.useState<LessonLearned[]>([]);
  // 表示するビューは URL ハッシュで決まる（サイドバーの `/#cot` などから切り替える）
  const hash = useHash();
  const activeTab = TAB_IDS.includes(hash) ? hash : "overview";

  // バックエンドからの全データ取得同期
  const syncWithBackend = React.useCallback(async () => {
    try {
      const [m, pRes, tRes, cRes, lRes] = await Promise.all([
        tradingApi.getStatus(),
        tradingApi.getPositions(),
        tradingApi.getTrades(),
        tradingApi.getCoTLogs(),
        tradingApi.getLessons(),
      ]);

      setMetrics(m);
      if (pRes.success && pRes.data) setPositions(pRes.data);
      if (tRes.success && tRes.data) setTrades(tRes.data);
      if (cRes.success && cRes.data) setCotLogs(cRes.data);
      if (lRes.success && lRes.data) setLessons(lRes.data);
      setEngineStatus("online");
    } catch {
      setEngineStatus("offline");
    }
  }, []);

  // 初回マウント & 5秒ごとの定期ポーリング同期（setState はタイマーコールバック内でのみ行う）
  React.useEffect(() => {
    const initial = setTimeout(syncWithBackend, 0);
    const interval = setInterval(syncWithBackend, 5000);

    return () => {
      clearTimeout(initial);
      clearInterval(interval);
    };
  }, [syncWithBackend]);

  // ボット稼働トグル切り替え
  const handleStateChange = async (newState: BotState) => {
    try {
      const res = await tradingApi.updateBotState(newState);
      if (!res.success) {
        toast.error("ボット状態の更新に失敗しました", { description: res.message });
        return;
      }
      if (newState === "running") {
        toast.success("自動売買エンジンを稼働開始しました", {
          description: "5分足確定ごとにLLM推論とリスクチェックが実行されます。",
        });
      } else {
        toast.warning("自動売買を一時停止しました", {
          description: "新規発注は停止されます（既存ポジションはブローカー側SL/TPで継続保護）。",
        });
      }
    } catch (err) {
      toast.error("ボット状態の更新に失敗しました", { description: errorMessage(err) });
    } finally {
      syncWithBackend();
    }
  };

  // 緊急全決済 & 停止
  const handleEmergencyStop = async () => {
    try {
      const res = await tradingApi.emergencyStop();
      toast.error("緊急全決済を執行しました", {
        description: res.data || res.message || "保有中のポジションを成行決済し、自動売買を停止しました。",
      });
    } catch (err) {
      toast.error("緊急全決済の送信に失敗しました", { description: errorMessage(err) });
    } finally {
      syncWithBackend();
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
      syncWithBackend();
    }
  };

  // 教訓のトグル
  const handleToggleLesson = async (id: string) => {
    try {
      const res = await tradingApi.toggleLesson(id);
      if (res.success && res.data) {
        const updated = res.data;
        setLessons((prev) => prev.map((l) => (l.id === id ? updated : l)));
        toast.info("教訓ルールの適用状態を更新しました");
      } else {
        toast.error("教訓ルールの更新に失敗しました", { description: res.message });
      }
    } catch (err) {
      toast.error("教訓ルールの更新に失敗しました", { description: errorMessage(err) });
    }
  };

  // 教訓の追加
  const handleAddLesson = async (
    newLessonData: Omit<LessonLearned, "id" | "createdAt">
  ) => {
    try {
      const res = await tradingApi.createLesson(newLessonData);
      if (res.success && res.data) {
        const created = res.data;
        setLessons((prev) => [created, ...prev]);
        toast.success("新しい教訓ルールを登録しました", {
          description: "次回のLLM推論プロンプトに禁止・注意事項として注入されます。",
        });
      } else {
        toast.error("教訓ルールの登録に失敗しました", { description: res.message });
      }
    } catch (err) {
      toast.error("教訓ルールの登録に失敗しました", { description: errorMessage(err) });
    }
  };

  // 教訓の削除
  const handleDeleteLesson = async (id: string) => {
    try {
      const res = await tradingApi.deleteLesson(id);
      if (res.success) {
        setLessons((prev) => prev.filter((l) => l.id !== id));
        toast.info("教訓ルールを削除しました");
      } else {
        toast.error("教訓ルールの削除に失敗しました", { description: res.message });
      }
    } catch (err) {
      toast.error("教訓ルールの削除に失敗しました", { description: errorMessage(err) });
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
            onRefresh={syncWithBackend}
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
          {/* リプレイ結果 タブ */}
          <TabsContent value="replay" className="space-y-4">
            <ReplayViewer />
          </TabsContent>

          {/* 全体概要タブ */}
          <TabsContent value="overview" className="space-y-4">
            <div className="grid grid-cols-1 xl:grid-cols-12 gap-6">
              {/* 左カラム: ポジション + チャート (7/12) */}
              <div className="xl:col-span-7 space-y-6">
                <PositionTable
                  positions={positions}
                  onClosePosition={handleClosePosition}
                />
                <ChartPreview />
              </div>

              {/* 右カラム: 条件付きプラン + LLM思考ログ (5/12) */}
              <div className="xl:col-span-5 space-y-6">
                <PlanMonitor onDecided={syncWithBackend} />
                <CoTViewer logs={cotLogs.slice(0, 3)} />
                <TradeHistoryTable trades={trades.slice(0, 3)} />
              </div>
            </div>
          </TabsContent>

          {/* LLM思考ログ タブ */}
          <TabsContent value="cot" className="space-y-4">
            <CoTViewer logs={cotLogs} />
          </TabsContent>

          {/* チャートプレビュー タブ */}
          <TabsContent value="chart" className="space-y-4">
            <ChartPreview />
          </TabsContent>

          {/* 約定履歴 タブ */}
          <TabsContent value="trades" className="space-y-4">
            <TradeHistoryTable trades={trades} />
          </TabsContent>

          {/* 教訓マネージャー タブ */}
          <TabsContent value="lessons" className="space-y-4">
            <LessonsManager
              lessons={lessons}
              onToggleLesson={handleToggleLesson}
              onAddLesson={handleAddLesson}
              onDeleteLesson={handleDeleteLesson}
            />
          </TabsContent>
        </Tabs>
      </div>
    </DashboardLayout>
  );
}
