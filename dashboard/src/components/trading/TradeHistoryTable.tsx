"use client";

import * as React from "react";
import { TradeHistory, CloseReason } from "@/types/trading";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { ArrowUpRight, ArrowDownRight, History, CheckCircle, AlertTriangle, Hand, BrainCircuit } from "lucide-react";
import { cn } from "@/lib/utils";
import { CoTDetailDialog } from "@/components/trading/CoTDetailDialog";
import { PaginationBar, PaginationBarProps } from "@/components/trading/PaginationBar";

interface TradeHistoryTableProps {
  trades: TradeHistory[];
  /** 指定時はカード下部にページ送りを表示（ページングはサーバー側） */
  pagination?: PaginationBarProps;
}

export function TradeHistoryTable({ trades, pagination }: TradeHistoryTableProps) {
  const [selectedCotId, setSelectedCotId] = React.useState<string | null>(null);

  const formatCurrency = (val: number) => {
    return new Intl.NumberFormat("ja-JP", {
      style: "currency",
      currency: "JPY",
      maximumFractionDigits: 0,
    }).format(val);
  };

  const getCloseReasonBadge = (reason: CloseReason) => {
    switch (reason) {
      case "TAKE_PROFIT":
        return (
          <Badge className="bg-emerald-600/15 text-emerald-700 dark:text-emerald-400 border-emerald-500/30 gap-1 text-[11px]">
            <CheckCircle className="h-3 w-3" />
            利確 (TP)
          </Badge>
        );
      case "STOP_LOSS":
        return (
          <Badge className="bg-rose-600/15 text-rose-700 dark:text-rose-400 border-rose-500/30 gap-1 text-[11px]">
            <AlertTriangle className="h-3 w-3" />
            損切り (SL)
          </Badge>
        );
      case "MANUAL":
        return (
          <Badge variant="outline" className="gap-1 text-[11px]">
            <Hand className="h-3 w-3" />
            手動決済
          </Badge>
        );
      case "CIRCUIT_BREAKER":
        return (
          <Badge className="bg-amber-600/15 text-amber-700 dark:text-amber-400 border-amber-500/30 gap-1 text-[11px]">
            強制停止
          </Badge>
        );
    }
  };

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-row items-center justify-between pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <History className="h-5 w-5 text-indigo-500" />
            <span>約定履歴 (Trade History)</span>
          </CardTitle>
          <CardDescription className="text-xs">
            決済済みトレードの結果と、エントリーの根拠になった判断
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent className="p-0">
        <div className="overflow-x-auto">
          <Table>
            <TableHeader className="bg-muted/40">
              <TableRow>
                <TableHead className="w-[100px]">銘柄</TableHead>
                <TableHead className="w-[80px]">売買</TableHead>
                <TableHead className="text-right">数量</TableHead>
                <TableHead className="text-right">エントリー</TableHead>
                <TableHead className="text-right">決済価格</TableHead>
                <TableHead className="text-right">損益 (pips)</TableHead>
                <TableHead className="text-right">確定損益</TableHead>
                <TableHead className="text-center">決済理由</TableHead>
                <TableHead className="text-right">決済時刻</TableHead>
                <TableHead className="text-center">判断</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {trades.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={10}
                    className="h-24 text-center text-muted-foreground text-sm"
                  >
                    決済済みのトレード履歴はありません
                  </TableCell>
                </TableRow>
              ) : (
                trades.map((trd) => {
                  const isPositive = trd.pnlPips >= 0;
                  return (
                    <TableRow key={trd.id} className="hover:bg-muted/30">
                      <TableCell className="font-bold text-sm">
                        {trd.symbol}
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant="outline"
                          className={cn(
                            "gap-1 font-semibold text-xs",
                            trd.side === "BUY"
                              ? "border-emerald-500/40 bg-emerald-50 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-400"
                              : "border-rose-500/40 bg-rose-50 text-rose-700 dark:bg-rose-950/40 dark:text-rose-400"
                          )}
                        >
                          {trd.side === "BUY" ? (
                            <ArrowUpRight className="h-3.5 w-3.5" />
                          ) : (
                            <ArrowDownRight className="h-3.5 w-3.5" />
                          )}
                          {trd.side}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-right font-mono tabular-nums text-sm">
                        {trd.volumeLots.toFixed(2)} lot
                      </TableCell>
                      <TableCell className="text-right font-mono tabular-nums text-sm">
                        {trd.entryPrice.toFixed(trd.symbol.includes("JPY") ? 3 : 5)}
                      </TableCell>
                      <TableCell className="text-right font-mono tabular-nums text-sm font-medium">
                        {trd.closePrice.toFixed(trd.symbol.includes("JPY") ? 3 : 5)}
                      </TableCell>
                      <TableCell
                        className={cn(
                          "text-right font-mono tabular-nums text-sm font-semibold",
                          isPositive
                            ? "text-emerald-600 dark:text-emerald-400"
                            : "text-rose-600 dark:text-rose-400"
                        )}
                      >
                        {isPositive ? "+" : ""}
                        {trd.pnlPips.toFixed(1)}
                      </TableCell>
                      <TableCell
                        className={cn(
                          "text-right font-mono tabular-nums text-sm font-bold",
                          isPositive
                            ? "text-emerald-600 dark:text-emerald-400"
                            : "text-rose-600 dark:text-rose-400"
                        )}
                      >
                        {isPositive ? "+" : ""}
                        {formatCurrency(trd.pnlAmount)}
                      </TableCell>
                      <TableCell className="text-center">
                        {getCloseReasonBadge(trd.closeReason)}
                      </TableCell>
                      <TableCell className="text-right text-xs text-muted-foreground font-mono tabular-nums">
                        {trd.closeTime}
                      </TableCell>

                      <TableCell className="text-center">
                        {trd.cotLogId ? (
                          <Button
                            variant="ghost"
                            size="sm"
                            className="h-7 gap-1 text-xs"
                            onClick={() => setSelectedCotId(trd.cotLogId ?? null)}
                          >
                            <BrainCircuit className="h-3.5 w-3.5" />
                            根拠
                          </Button>
                        ) : (
                          <span className="text-xs text-muted-foreground">—</span>
                        )}
                      </TableCell>
                    </TableRow>
                  );
                })
              )}
            </TableBody>
          </Table>
        </div>
        {pagination && (
          <div className="px-4 py-3 border-t">
            <PaginationBar {...pagination} />
          </div>
        )}
      </CardContent>

      <CoTDetailDialog cotLogId={selectedCotId} onClose={() => setSelectedCotId(null)} />
    </Card>
  );
}
