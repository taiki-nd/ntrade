"use client";

import * as React from "react";
import { AccountMetrics } from "@/types/trading";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Wallet,
  TrendingUp,
  TrendingDown,
  Layers,
  Percent,
  ShieldCheck,
} from "lucide-react";
import { cn } from "@/lib/utils";

interface MetricsCardsProps {
  metrics: AccountMetrics;
}

export function MetricsCards({ metrics }: MetricsCardsProps) {
  const formatCurrency = (val: number) => {
    return new Intl.NumberFormat("ja-JP", {
      style: "currency",
      currency: "JPY",
      maximumFractionDigits: 0,
    }).format(val);
  };

  const isDailyPnlPositive = metrics.dailyPnl >= 0;
  const isUnrealizedPositive = metrics.unrealizedPnl >= 0;

  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-2 lg:grid-cols-5">
      {/* 1. 口座純資産 / 残高 */}
      <Card className="shadow-xs">
        <CardHeader className="flex flex-row items-center justify-between pb-2 space-y-0">
          <CardTitle className="text-xs font-medium text-muted-foreground">
            口座純資産 / 残高
          </CardTitle>
          <Wallet className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          <div className="text-xl font-bold tracking-tight">
            {formatCurrency(metrics.equity)}
          </div>
          <p className="text-xs text-muted-foreground mt-1">
            {metrics.orderMode === "live" ? "ブローカー残高" : "ペーパー残高"}: {formatCurrency(metrics.balance)}
          </p>
          {metrics.orderMode !== "live" && metrics.brokerBalance !== undefined && (
            <p className="text-xs text-muted-foreground">
              cTrader 口座: {formatCurrency(metrics.brokerBalance)}
            </p>
          )}
        </CardContent>
      </Card>

      {/* 2. 本日の確定損益 */}
      <Card className="shadow-xs">
        <CardHeader className="flex flex-row items-center justify-between pb-2 space-y-0">
          <CardTitle className="text-xs font-medium text-muted-foreground">
            本日の確定損益
          </CardTitle>
          {isDailyPnlPositive ? (
            <TrendingUp className="h-4 w-4 text-emerald-500" />
          ) : (
            <TrendingDown className="h-4 w-4 text-rose-500" />
          )}
        </CardHeader>
        <CardContent>
          <div
            className={cn(
              "text-xl font-bold tracking-tight",
              isDailyPnlPositive ? "text-emerald-600 dark:text-emerald-400" : "text-rose-600 dark:text-rose-400"
            )}
          >
            {isDailyPnlPositive ? "+" : ""}
            {formatCurrency(metrics.dailyPnl)}
          </div>
          <p
            className={cn(
              "text-xs font-medium mt-1",
              isDailyPnlPositive ? "text-emerald-600 dark:text-emerald-400" : "text-rose-600 dark:text-rose-400"
            )}
          >
            {isDailyPnlPositive ? "+" : ""}
            {metrics.dailyPnlPercent.toFixed(2)}%
          </p>
        </CardContent>
      </Card>

      {/* 3. 保有含み損益 */}
      <Card className="shadow-xs">
        <CardHeader className="flex flex-row items-center justify-between pb-2 space-y-0">
          <CardTitle className="text-xs font-medium text-muted-foreground">
            保有含み損益
          </CardTitle>
          <Layers className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          <div
            className={cn(
              "text-xl font-bold tracking-tight",
              isUnrealizedPositive ? "text-emerald-600 dark:text-emerald-400" : "text-rose-600 dark:text-rose-400"
            )}
          >
            {isUnrealizedPositive ? "+" : ""}
            {formatCurrency(metrics.unrealizedPnl)}
          </div>
          <p className="text-xs text-muted-foreground mt-1">
            使用証拠金: {formatCurrency(metrics.margin)}
          </p>
        </CardContent>
      </Card>

      {/* 4. 本日勝率 */}
      <Card className="shadow-xs">
        <CardHeader className="flex flex-row items-center justify-between pb-2 space-y-0">
          <CardTitle className="text-xs font-medium text-muted-foreground">
            本日勝率
          </CardTitle>
          <Percent className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          <div className="text-xl font-bold tracking-tight">
            {metrics.winRateToday.toFixed(1)}%
          </div>
          <p className="text-xs text-muted-foreground mt-1">
            {metrics.winningTradesToday}勝 {metrics.totalTradesToday - metrics.winningTradesToday}敗 ({metrics.totalTradesToday}回)
          </p>
        </CardContent>
      </Card>

      {/* 5. スプレッド & 安全ガード */}
      <Card className="shadow-xs col-span-2 lg:col-span-1">
        <CardHeader className="flex flex-row items-center justify-between pb-2 space-y-0">
          <CardTitle className="text-xs font-medium text-muted-foreground">
            スプレッド / 防御ガード
          </CardTitle>
          <ShieldCheck className="h-4 w-4 text-emerald-500" />
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between text-sm font-semibold">
            <span className="text-muted-foreground">USD/JPY:</span>
            <span className="text-emerald-600 dark:text-emerald-400">{metrics.usdjpySpread} pips</span>
          </div>
          <div className="flex items-center justify-between text-xs text-muted-foreground mt-1">
            <span>EUR/USD: {metrics.eurusdSpread} pips</span>
            <span>日次上限: {metrics.circuitBreakerThresholdPercent}%</span>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
