"use client";

import * as React from "react";
import { DashboardLayout } from "@/components/layouts/dashboard-layout";
import { BotControlHeader } from "@/components/trading/BotControlHeader";
import { MetricsCards } from "@/components/trading/MetricsCards";
import { PositionTable } from "@/components/trading/PositionTable";
import { CoTViewer } from "@/components/trading/CoTViewer";
import { ChartPreview } from "@/components/trading/ChartPreview";
import { TradeHistoryTable } from "@/components/trading/TradeHistoryTable";
import { LessonsManager } from "@/components/trading/LessonsManager";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  initialMetrics,
  initialPositions,
  initialTrades,
  initialCoTLogs,
  initialLessons,
} from "@/lib/mock-trading-data";
import {
  BotState,
  AccountMetrics,
  Position,
  TradeHistory,
  CoTLog,
  LessonLearned,
} from "@/types/trading";
import { toast } from "sonner";
import {
  LayoutDashboard,
  BrainCircuit,
  LineChart,
  History,
  Lightbulb,
  ShieldCheck,
} from "lucide-react";

export default function TradingDashboard() {
  const [botState, setBotState] = React.useState<BotState>("running");
  const [metrics, setMetrics] = React.useState<AccountMetrics>(initialMetrics);
  const [positions, setPositions] = React.useState<Position[]>(initialPositions);
  const [trades, setTrades] = React.useState<TradeHistory[]>(initialTrades);
  const [cotLogs, setCotLogs] = React.useState<CoTLog[]>(initialCoTLogs);
  const [lessons, setLessons] = React.useState<LessonLearned[]>(initialLessons);
  const [activeTab, setActiveTab] = React.useState<string>("overview");

  // ボット稼働トグル切り替え
  const handleStateChange = (newState: BotState) => {
    setBotState(newState);
    if (newState === "running") {
      toast.success("自動売買エンジンを稼働開始しました", {
        description: "5分足確定ごとにLLM推論とリスクチェックが実行されます。",
      });
    } else if (newState === "paused") {
      toast.warning("自動売買を一時停止しました", {
        description: "新規発注は停止されます（既存ポジションはブローカー側SL/TPで継続保護）。",
      });
    }
  };

  // 緊急全決済 & 停止
  const handleEmergencyStop = () => {
    if (positions.length === 0) {
      setBotState("paused");
      toast.info("保有ポジションはありませんでした。ボットを一時停止しました。");
      return;
    }

    const nowStr = new Date().toISOString().replace("T", " ").substring(0, 19);
    let totalPnlAmount = 0;

    // 全ポジションをTradeHistoryへ移動
    const closedTrades: TradeHistory[] = positions.map((p, idx) => {
      totalPnlAmount += p.pnlAmount;
      return {
        id: `trd-emerg-${Date.now()}-${idx}`,
        symbol: p.symbol,
        side: p.side,
        volumeLots: p.volumeLots,
        entryPrice: p.entryPrice,
        closePrice: p.currentPrice,
        stopLoss: p.stopLoss,
        takeProfit: p.takeProfit,
        pnlPips: p.pnlPips,
        pnlAmount: p.pnlAmount,
        closeReason: "MANUAL",
        openTime: p.openTime,
        closeTime: nowStr,
      };
    });

    setTrades((prev) => [...closedTrades, ...prev]);
    setPositions([]);
    setBotState("paused");
    setMetrics((prev) => ({
      ...prev,
      balance: prev.balance + totalPnlAmount,
      equity: prev.balance + totalPnlAmount,
      unrealizedPnl: 0,
      margin: 0,
      freeMargin: prev.balance + totalPnlAmount,
      dailyPnl: prev.dailyPnl + totalPnlAmount,
      totalTradesToday: prev.totalTradesToday + closedTrades.length,
      winningTradesToday:
        prev.winningTradesToday + closedTrades.filter((t) => t.pnlPips > 0).length,
    }));

    toast.error("緊急全決済を執行しました", {
      description: `${closedTrades.length} 件のポジションを成行決済し、ボットを一時停止しました。`,
    });
  };

  // 単一ポジションの手動決済
  const handleClosePosition = (id: string) => {
    const target = positions.find((p) => p.id === id);
    if (!target) return;

    const nowStr = new Date().toISOString().replace("T", " ").substring(0, 19);
    const newTrade: TradeHistory = {
      id: `trd-man-${Date.now()}`,
      symbol: target.symbol,
      side: target.side,
      volumeLots: target.volumeLots,
      entryPrice: target.entryPrice,
      closePrice: target.currentPrice,
      stopLoss: target.stopLoss,
      takeProfit: target.takeProfit,
      pnlPips: target.pnlPips,
      pnlAmount: target.pnlAmount,
      closeReason: "MANUAL",
      openTime: target.openTime,
      closeTime: nowStr,
    };

    setPositions((prev) => prev.filter((p) => p.id !== id));
    setTrades((prev) => [newTrade, ...prev]);
    setMetrics((prev) => ({
      ...prev,
      balance: prev.balance + target.pnlAmount,
      equity: prev.equity,
      unrealizedPnl: prev.unrealizedPnl - target.pnlAmount,
      dailyPnl: prev.dailyPnl + target.pnlAmount,
      totalTradesToday: prev.totalTradesToday + 1,
      winningTradesToday:
        prev.winningTradesToday + (target.pnlPips > 0 ? 1 : 0),
    }));

    toast.success(`${target.symbol} ポジションを手動成行決済しました`, {
      description: `損益: ${target.pnlPips >= 0 ? "+" : ""}${target.pnlPips.toFixed(1)} pips (¥${target.pnlAmount.toLocaleString()})`,
    });
  };

  // 教訓のトグル
  const handleToggleLesson = (id: string) => {
    setLessons((prev) =>
      prev.map((l) => (l.id === id ? { ...l, active: !l.active } : l))
    );
    toast.info("教訓ルールの適用状態を更新しました");
  };

  // 教訓の追加
  const handleAddLesson = (
    newLessonData: Omit<LessonLearned, "id" | "createdAt">
  ) => {
    const newLesson: LessonLearned = {
      ...newLessonData,
      id: `les-${Date.now()}`,
      createdAt: new Date().toISOString().replace("T", " ").substring(0, 19),
    };
    setLessons((prev) => [newLesson, ...prev]);
    toast.success("新しい教訓ルールを登録しました", {
      description: "次回のLLM推論プロンプトに禁止・注意事項として注入されます。",
    });
  };

  // 教訓の削除
  const handleDeleteLesson = (id: string) => {
    setLessons((prev) => prev.filter((l) => l.id !== id));
    toast.info("教訓ルールを削除しました");
  };

  return (
    <DashboardLayout title="ntrade 自動売買ダッシュボード">
      <div className="space-y-6">
        {/* 1. 稼働ステータス & 緊急停止コントロールヘッダー */}
        <BotControlHeader
          botState={botState}
          metrics={metrics}
          onStateChange={handleStateChange}
          onEmergencyStop={handleEmergencyStop}
        />

        {/* 2. サマリーメトリクスカード */}
        <MetricsCards metrics={metrics} />

        {/* 3. メインビュータブ */}
        <Tabs
          value={activeTab}
          onValueChange={setActiveTab}
          className="space-y-4"
        >
          <TabsList className="grid w-full grid-cols-5 max-w-2xl bg-muted/60">
            <TabsTrigger value="overview" className="gap-1.5 text-xs font-medium cursor-pointer">
              <LayoutDashboard className="h-4 w-4" />
              全体概要
            </TabsTrigger>
            <TabsTrigger value="cot" className="gap-1.5 text-xs font-medium cursor-pointer">
              <BrainCircuit className="h-4 w-4" />
              LLM思考ログ
            </TabsTrigger>
            <TabsTrigger value="chart" className="gap-1.5 text-xs font-medium cursor-pointer">
              <LineChart className="h-4 w-4" />
              チャート
            </TabsTrigger>
            <TabsTrigger value="trades" className="gap-1.5 text-xs font-medium cursor-pointer">
              <History className="h-4 w-4" />
              約定履歴
            </TabsTrigger>
            <TabsTrigger value="lessons" className="gap-1.5 text-xs font-medium cursor-pointer">
              <Lightbulb className="h-4 w-4" />
              教訓ルール
            </TabsTrigger>
          </TabsList>

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

              {/* 右カラム: LLM思考ログ (5/12) */}
              <div className="xl:col-span-5 space-y-6">
                <CoTViewer logs={cotLogs} />
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
