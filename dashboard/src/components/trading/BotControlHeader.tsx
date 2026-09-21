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
import { Separator } from "@/components/ui/separator";
import { tradingApi } from "@/lib/trading-api";
import { cn } from "@/lib/utils";
import { toast } from "sonner";

const yen = new Intl.NumberFormat("ja-JP", {
  style: "currency",
  currency: "JPY",
  maximumFractionDigits: 0,
});
const signedYen = new Intl.NumberFormat("ja-JP", {
  style: "currency",
  currency: "JPY",
  maximumFractionDigits: 0,
  signDisplay: "exceptZero",
});

const formatCurrency = (val: number, signed = false) => (signed ? signedYen : yen).format(val);

interface StatProps {
  label: string;
  value: string;
  sub?: string;
  valueClassName?: string;
  title?: string;
}

/** ヘッダー内の1指標（ラベル + 値 + 補足）。縦幅を抑えるため2行に収める */
function Stat({ label, value, sub, valueClassName, title }: StatProps) {
  return (
    <div className="flex flex-col leading-tight" title={title}>
      <span className="text-[11px] font-medium text-muted-foreground">{label}</span>
      <span className="flex items-baseline gap-1.5 whitespace-nowrap">
        <span className={cn("text-sm font-semibold font-mono tabular-nums tracking-tight", valueClassName)}>{value}</span>
        {sub && <span className="text-[11px] font-mono tabular-nums text-muted-foreground">{sub}</span>}
      </span>
    </div>
  );
}

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
          <div className="flex items-center gap-1.5" title="AUTO RUNNING">
            <span className="h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
            <span className="text-sm font-semibold text-emerald-600 dark:text-emerald-400">稼働中</span>
          </div>
        );
      case "paused":
        return (
          <div className="flex items-center gap-1.5" title="PAUSED">
            <span className="h-2 w-2 rounded-full bg-amber-500" />
            <span className="text-sm font-semibold text-amber-600 dark:text-amber-400">一時停止中</span>
          </div>
        );
      case "circuit_breaker":
        return (
          <div className="flex items-center gap-1.5" title="SHUTDOWN">
            <span className="h-2 w-2 rounded-full bg-rose-600" />
            <span className="text-sm font-semibold text-rose-600 dark:text-rose-400">サーキットブレーカー</span>
          </div>
        );
    }
  };

  const isCtraderConnected = metrics.connectionStatus.ctrader === "connected";
  const pnlClass = (v: number) =>
    v > 0 ? "text-emerald-600 dark:text-emerald-400" : v < 0 ? "text-rose-600 dark:text-rose-400" : undefined;
  const balanceTitle = [
    `${metrics.orderMode === "live" ? "ブローカー残高" : "ペーパー残高"}: ${formatCurrency(metrics.balance)}`,
    metrics.orderMode !== "live" && metrics.brokerBalance !== undefined
      ? `cTrader 口座: ${formatCurrency(metrics.brokerBalance)}`
      : null,
  ]
    .filter(Boolean)
    .join("\n");

  return (
    <>
      <div className="flex min-w-0 flex-1 items-center justify-between gap-4 py-1.5 overflow-x-auto scrollbar-none">
        {/* 左側: ステータス + 口座サマリー */}
        <div className="flex items-center gap-x-4 lg:gap-x-5 min-w-0 shrink-0">
          <div className="shrink-0">{getStatusBadge()}</div>

          <Separator orientation="vertical" className="hidden h-7 shrink-0 sm:block" />

          {/* 口座サマリー */}
          <div className="flex items-center gap-x-4 lg:gap-x-5">
            <Stat label="純資産" value={formatCurrency(metrics.equity)} title={balanceTitle} />
            <Stat
              label="本日確定"
              value={formatCurrency(metrics.dailyPnl, true)}
              sub={`${metrics.dailyPnl > 0 ? "+" : ""}${metrics.dailyPnlPercent.toFixed(2)}%`}
              valueClassName={pnlClass(metrics.dailyPnl)}
            />
            <Stat
              label="含み損益"
              value={formatCurrency(metrics.unrealizedPnl, true)}
              sub={`証拠金 ${formatCurrency(metrics.margin)}`}
              valueClassName={pnlClass(metrics.unrealizedPnl)}
            />
            <Stat
              label="本日勝率"
              value={`${metrics.winRateToday.toFixed(1)}%`}
              sub={`${metrics.winningTradesToday}勝${metrics.totalTradesToday - metrics.winningTradesToday}敗`}
            />
            <Stat
              label="スプレッド"
              value={`UJ ${metrics.usdjpySpread} / EU ${metrics.eurusdSpread}`}
              sub="pips"
            />
          </div>
        </div>

        {/* 右側: 接続状態 & コントロール */}
        <div className="ml-auto flex items-center gap-2 shrink-0">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowOAuthDialog(true)}
            title={`cTrader連携設定${metrics.connectionStatus.accountNumber ? ` / ${metrics.connectionStatus.accountNumber}` : ""}`}
            className="h-8 gap-1.5 text-xs text-muted-foreground hover:text-foreground cursor-pointer"
          >
            <Server className={`h-3.5 w-3.5 ${isCtraderConnected ? "text-emerald-500" : "text-rose-500"}`} />
            {isCtraderConnected ? (
              metrics.connectionStatus.pingMs > 0
                ? `${metrics.connectionStatus.environment} (${metrics.connectionStatus.pingMs}ms)`
                : metrics.connectionStatus.environment
            ) : (
              <span className="text-rose-500 font-medium">cTrader未接続</span>
            )}
          </Button>

          <div
            className="hidden items-center gap-1.5 px-2 py-1 rounded-md bg-muted/60 text-xs font-mono text-muted-foreground xl:flex"
            title="LLM Pipeline: claude -p / agy -p"
          >
            <Cpu className="h-3.5 w-3.5 text-primary" />
            <span>{metrics.connectionStatus.llm}</span>
          </div>

          {botState === "running" ? (
            <Button
              variant="outline"
              size="sm"
              onClick={() => onStateChange("paused")}
              className="h-8 text-xs text-amber-600 border-amber-300 hover:bg-amber-50 dark:border-amber-800 dark:hover:bg-amber-950/40 cursor-pointer"
            >
              <Pause className="h-3.5 w-3.5" />
              一時停止
            </Button>
          ) : (
            <Button
              variant="default"
              size="sm"
              onClick={() => onStateChange("running")}
              className="h-8 text-xs bg-emerald-600 hover:bg-emerald-700 text-white cursor-pointer"
            >
              <Play className="h-3.5 w-3.5" />
              自動売買開始
            </Button>
          )}

          <Button
            variant="destructive"
            size="sm"
            onClick={() => setShowEmergencyDialog(true)}
            className="h-8 text-xs font-semibold cursor-pointer"
          >
            <AlertOctagon className="h-3.5 w-3.5" />
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
              ※ ボタンをクリックすると Spotware 社の認可画面へ遷移します。ログイン・承認後、本管理画面へ自動で戻り、口座が即時同期されます。
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
    </>
  );
}

