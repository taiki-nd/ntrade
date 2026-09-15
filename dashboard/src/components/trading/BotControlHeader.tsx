"use client";

import * as React from "react";
import { BotState, AccountMetrics } from "@/types/trading";
import { Button } from "@/components/ui/button";
import {
  Play,
  Pause,
  AlertOctagon,
  Cpu,
  ShieldAlert,
  Server,
  Link2,
  ExternalLink,
  Loader2,
  CheckCircle2,
} from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { tradingApi } from "@/lib/trading-api";
import { toast } from "sonner";

interface BotControlHeaderProps {
  botState: BotState;
  metrics: AccountMetrics;
  onStateChange: (newState: BotState) => void;
  onEmergencyStop: () => void;
  onRefresh?: () => void;
}

export function BotControlHeader({
  botState,
  metrics,
  onStateChange,
  onEmergencyStop,
}: BotControlHeaderProps) {
  const [showEmergencyDialog, setShowEmergencyDialog] = React.useState(false);
  const [showOAuthDialog, setShowOAuthDialog] = React.useState(false);
  const [isLinking, setIsLinking] = React.useState(false);

  // cTrader OAuth 画面へのリダイレクト
  const handleStartOAuth = async () => {
    try {
      setIsLinking(true);
      const res = await tradingApi.getOAuthUrl();
      if (res.success && res.data?.url) {
        toast.info("Spotware 認可画面を開きます...", {
          description: "承認後に自動でローカル管理画面へ戻ります。",
        });
        window.location.href = res.data.url;
      } else {
        toast.error("OAuth URLの取得に失敗しました", {
          description: res.message || ".env の CTRADER_CLIENT_ID を確認してください。",
        });
      }
    } catch (err: unknown) {
      toast.error("Rustコアエンジン (localhost:4000) への接続に失敗しました", {
        description: err instanceof Error ? err.message : "エンジンが起動しているか確認してください。",
      });
    } finally {
      setIsLinking(false);
    }
  };

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

  const isCtraderConnected = metrics.connectionStatus.ctrader === "connected";

  return (
    <div className="rounded-xl border bg-card p-4 shadow-sm">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        {/* 左側: ステータスと接続情報 */}
        <div className="flex flex-wrap items-center gap-4">
          <div className="flex items-center gap-3 pr-4 border-r">
            {getStatusBadge()}
          </div>

          <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            {/* cTrader 接続ステータス */}
            <div className="flex items-center gap-1.5 rounded-md bg-muted/60 px-2.5 py-1">
              <Server
                className={`h-3.5 w-3.5 ${
                  isCtraderConnected ? "text-emerald-500" : "text-rose-500"
                }`}
              />
              <span>cTrader:</span>
              <span className="font-medium text-foreground">
                {isCtraderConnected ? (
                  `${metrics.connectionStatus.environment} (${metrics.connectionStatus.pingMs}ms)`
                ) : (
                  <span className="text-rose-500">未接続</span>
                )}
              </span>
            </div>

            {/* cTrader OAuth 連携ボタン */}
            <Button
              variant={isCtraderConnected ? "ghost" : "outline"}
              size="sm"
              onClick={() => setShowOAuthDialog(true)}
              className={`h-7 px-2.5 gap-1.5 text-xs font-medium cursor-pointer ${
                !isCtraderConnected
                  ? "border-emerald-500 text-emerald-600 hover:bg-emerald-50 dark:hover:bg-emerald-950/40"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <Link2 className="h-3.5 w-3.5" />
              {isCtraderConnected ? "cTrader連携設定" : "cTraderを連携"}
            </Button>

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

      {/* cTrader OAuth 連携モーダル */}
      <Dialog open={showOAuthDialog} onOpenChange={setShowOAuthDialog}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-primary">
              <Link2 className="h-5 w-5 text-emerald-500" />
              cTrader Open API アプリ内連携
            </DialogTitle>
            <DialogDescription>
              Spotware 社の公式 OAuth 2.0 認可画面を開き、ワンクリックで口座認証と自動トークン更新を設定します。手動でのトークンコピーは不要です。
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-3 py-2 text-sm text-muted-foreground">
            <div className="rounded-lg border bg-muted/40 p-3 space-y-1.5">
              <div className="flex items-center gap-2 text-foreground font-medium">
                <CheckCircle2 className="h-4 w-4 text-emerald-500" />
                現在の接続口座:
              </div>
              <p className="text-xs pl-6">
                {metrics.connectionStatus.accountNumber || "未接続"}
              </p>
              <div className="flex items-center gap-2 text-foreground font-medium pt-1">
                <CheckCircle2 className="h-4 w-4 text-emerald-500" />
                環境:
              </div>
              <p className="text-xs pl-6">
                {metrics.connectionStatus.environment || "DEMO"} モード
              </p>
            </div>
            <p className="text-xs">
              ※ ボタンをクリックすると Spotware 社の認可画面へ遷移します。ログイン・承認後、本管理画面へ自動で戻り口座が即時同期されます。
            </p>
          </div>

          <DialogFooter className="gap-2 sm:gap-0">
            <Button
              variant="outline"
              onClick={() => setShowOAuthDialog(false)}
            >
              閉じる
            </Button>
            <Button
              className="gap-2 bg-emerald-600 hover:bg-emerald-700 text-white cursor-pointer"
              disabled={isLinking}
              onClick={handleStartOAuth}
            >
              {isLinking ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  準備中...
                </>
              ) : (
                <>
                  <ExternalLink className="h-4 w-4" />
                  Spotware 認可画面を開く
                </>
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
