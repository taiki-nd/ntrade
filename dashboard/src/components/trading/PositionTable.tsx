"use client";

import * as React from "react";
import { Position } from "@/types/trading";
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
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ArrowUpRight, ArrowDownRight, XCircle } from "lucide-react";
import { cn } from "@/lib/utils";

interface PositionTableProps {
  positions: Position[];
  onClosePosition: (id: string) => void;
}

export function PositionTable({ positions, onClosePosition }: PositionTableProps) {
  const formatCurrency = (val: number) => {
    return new Intl.NumberFormat("ja-JP", {
      style: "currency",
      currency: "JPY",
      maximumFractionDigits: 0,
    }).format(val);
  };

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-row items-center justify-between pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <span>保有ポジション</span>
            <Badge variant="secondary" className="font-mono text-xs">
              {positions.length} 件
            </Badge>
          </CardTitle>
          <p className="text-xs text-muted-foreground">
            ブローカー側でサーバーサイドSL/TPが常時稼働中
          </p>
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
                <TableHead className="text-right">現在値</TableHead>
                <TableHead className="text-right">SL / TP</TableHead>
                <TableHead className="text-right">損益 (pips)</TableHead>
                <TableHead className="text-right">評価損益</TableHead>
                <TableHead className="text-center w-[90px]">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {positions.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={9}
                    className="h-24 text-center text-muted-foreground text-sm"
                  >
                    現在保有中のポジションはありません（待機中）
                  </TableCell>
                </TableRow>
              ) : (
                positions.map((pos) => {
                  const isPositive = pos.pnlPips >= 0;
                  return (
                    <TableRow key={pos.id} className="hover:bg-muted/30">
                      <TableCell className="font-bold text-sm">
                        {pos.symbol}
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant="outline"
                          className={cn(
                            "gap-1 font-semibold text-xs",
                            pos.side === "BUY"
                              ? "border-emerald-500/40 bg-emerald-50 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-400"
                              : "border-rose-500/40 bg-rose-50 text-rose-700 dark:bg-rose-950/40 dark:text-rose-400"
                          )}
                        >
                          {pos.side === "BUY" ? (
                            <ArrowUpRight className="h-3.5 w-3.5" />
                          ) : (
                            <ArrowDownRight className="h-3.5 w-3.5" />
                          )}
                          {pos.side}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-right font-mono tabular-nums text-sm">
                        {pos.volumeLots.toFixed(2)} lot
                      </TableCell>
                      <TableCell className="text-right font-mono tabular-nums text-sm">
                        {pos.entryPrice.toFixed(pos.symbol.includes("JPY") ? 3 : 5)}
                      </TableCell>
                      <TableCell className="text-right font-mono tabular-nums text-sm font-semibold">
                        {pos.currentPrice.toFixed(pos.symbol.includes("JPY") ? 3 : 5)}
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex flex-col items-end gap-0.5">
                          <span className="font-mono tabular-nums text-xs text-rose-600 dark:text-rose-400">
                            SL: {pos.stopLoss.toFixed(pos.symbol.includes("JPY") ? 3 : 5)}
                          </span>
                          <span className="font-mono tabular-nums text-xs text-emerald-600 dark:text-emerald-400">
                            TP: {pos.takeProfit.toFixed(pos.symbol.includes("JPY") ? 3 : 5)}
                          </span>
                        </div>
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
                        {pos.pnlPips.toFixed(1)}
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
                        {formatCurrency(pos.pnlAmount)}
                      </TableCell>

                      <TableCell className="text-center">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => onClosePosition(pos.id)}
                          className="h-8 px-2 text-rose-600 hover:text-rose-700 hover:bg-rose-50 dark:hover:bg-rose-950/40 cursor-pointer"
                          title="このポジションを成行決済"
                        >
                          <XCircle className="h-4 w-4 mr-1" />
                          決済
                        </Button>
                      </TableCell>
                    </TableRow>
                  );
                })
              )}
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>
  );
}
