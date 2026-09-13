"use client";

import * as React from "react";
import { BotState, AccountMetrics } from "@/types/trading";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Play,
  Pause,
  AlertOctagon,
  Activity,
  Cpu,
  ShieldAlert,
  Server,
  RefreshCw,
} from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface BotControlHeaderProps {
  botState: BotState;
  metrics: AccountMetrics;
  onStateChange: (newState: BotState) => void;
  onEmergencyStop: () => void;
}

export function BotControlHeader({
  botState,
  metrics,
  onStateChange,
  onEmergencyStop,
}: BotControlHeaderProps) {
  const [showEmergencyDialog, setShowEmergencyDialog] = React.useState(false);

  const getStatusBadge = () => {
    switch (botState) {
      case "running":
        return (
          <div className="flex items-center gap-2">
            <span className="relative flex h-3 w-3">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
              <span className="relative inline-flex rounded-full h-3 w-3 bg-emerald-500"></span>
            </span>
            <span className="font-semibold text-emerald-600 dark:text-emerald-400">
              稼働中 (AUTO RUNNING)
            </span>
          </div>
        );
      case "paused":
        return (
          <div className="flex items-center gap-2">
            <span className="h-3 w-3 rounded-full bg-amber-500" />
            <span className="font-semibold text-amber-600 dark:text-amber-400">
              一時停止中 (PAUSED)
            </span>
          </div>
        );
      case "circuit_breaker":
        return (
          <div className="flex items-center gap-2">
            <span className="h-3 w-3 rounded-full bg-rose-600 animate-pulse" />
            <span className="font-semibold text-rose-600 dark:text-rose-400">
              サーキットブレーカー発動 (SHUTDOWN)
            </span>
          </div>
        );
    }
  };

  return (
    <div className="rounded-xl border bg-card p-4 shadow-sm">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        {/* 左側: ステータスと接続情報 */}
        <div className="flex flex-wrap items-center gap-4">
          <div className="flex items-center gap-3 pr-4 border-r">
            {getStatusBadge()}
          </div>

          <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            {/* cTrader 接続 */}
            <div className="flex items-center gap-1.5 rounded-md bg-muted/60 px-2.5 py-1">
              <Server className="h-3.5 w-3.5 text-emerald-500" />
              <span>cTrader:</span>
              <span className="font-medium text-foreground">
                {metrics.connectionStatus.environment} ({metrics.connectionStatus.pingMs}ms)
              </span>
            </div>

            {/* LLM 推論エンジン */}
            <div className="flex items-center gap-1.5 rounded-md bg-muted/60 px-2.5 py-1">
              <Cpu className="h-3.5 w-3.5 text-indigo-500" />
              <span>LLM Pipeline:</span>
              <span className="font-medium text-foreground">
                claude -p / agy -p (Ready)
              </span>
            </div>

            {/* 口座番号 */}
            <div className="hidden sm:flex items-center gap-1.5 rounded-md bg-muted/60 px-2.5 py-1">
              <span className="text-muted-foreground">{metrics.connectionStatus.accountNumber}</span>
            </div>
          </div>
        </div>

        {/* 右側: コントロールボタン群 */}
        <div className="flex items-center gap-2">
          {botState === "running" ? (
            <Button
              variant="outline"
              size="sm"
              onClick={() => onStateChange("paused")}
              className="gap-1.5 text-amber-600 border-amber-300 hover:bg-amber-50 dark:border-amber-800 dark:hover:bg-amber-950/40 cursor-pointer"
            >
              <Pause className="h-4 w-4" />
              一時停止
            </Button>
          ) : (
            <Button
              variant="default"
              size="sm"
              onClick={() => onStateChange("running")}
              className="gap-1.5 bg-emerald-600 hover:bg-emerald-700 text-white cursor-pointer"
            >
              <Play className="h-4 w-4" />
              自動売買開始
            </Button>
          )}

          <Button
            variant="destructive"
            size="sm"
            onClick={() => setShowEmergencyDialog(true)}
            className="gap-1.5 cursor-pointer font-semibold shadow-xs"
          >
            <AlertOctagon className="h-4 w-4" />
            緊急全決済
          </Button>
        </div>
      </div>

      {/* 緊急停止確認ダイアログ */}
      <Dialog open={showEmergencyDialog} onOpenChange={setShowEmergencyDialog}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-destructive">
              <ShieldAlert className="h-5 w-5" />
              緊急全決済 & システム停止の確認
            </DialogTitle>
            <DialogDescription>
              保有中のすべてのポジションを成行で即時決済し、ボットの新規自動発注を直ちに停止します。この操作は取り消せません。よろしいですか？
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="gap-2 sm:gap-0">
            <Button
              variant="outline"
              onClick={() => setShowEmergencyDialog(false)}
            >
              キャンセル
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                onEmergencyStop();
                setShowEmergencyDialog(false);
              }}
            >
              はい、直ちに全決済して停止
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
