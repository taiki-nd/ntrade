"use client";

import { priceDigits } from "@/lib/price";
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
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ArrowUpRight, ArrowDownRight, Radio, Wallet, XCircle } from "lucide-react";
import { cn } from "@/lib/utils";
import { isSignalId } from "@/lib/strategy";
import { SignalDetailDialog } from "@/components/trading/SignalViewer";
import { useNow } from "@/hooks/use-now";
import { formatDuration, parseEngineTime } from "@/lib/time";

const yen = new Intl.NumberFormat("ja-JP", { style: "currency", currency: "JPY", maximumFractionDigits: 0, signDisplay: "exceptZero" });

/** BUY / SELL の向き（建玉・予約注文で共通） */
export function SideBadge({ side, suffix }: { side: "BUY" | "SELL"; suffix?: string }) {
  return (
    <Badge
      variant="outline"
      className={cn(
        "gap-1 font-semibold text-xs",
        side === "BUY" ? "border-profit/40 bg-profit/10 text-profit" : "border-loss/40 bg-loss/10 text-loss"
      )}
    >
      {side === "BUY" ? <ArrowUpRight className="h-3.5 w-3.5" /> : <ArrowDownRight className="h-3.5 w-3.5" />}
      {side}
      {suffix && <span className="font-normal">{suffix}</span>}
    </Badge>
  );
}

/** 管理している戦略（デプロイ ID）。無ければ手動・ブローカーから取り込んだもの */
export function StrategyBadge({ strategy }: { strategy?: string | null }) {
  return strategy ? (
    <Badge variant="secondary" className="font-mono text-[11px]" title={`デプロイ ${strategy} が管理`}>
      {strategy}
    </Badge>
  ) : (
    <Badge variant="outline" className="text-[11px] text-muted-foreground" title="戦略の管理外（手動・ブローカーから取り込み）">
      手動/取込
    </Badge>
  );
}

/** SL / TP の価格。0 は未設定 */
export function formatLevel(v: number, digits: number): string {
  return v > 0 ? v.toFixed(digits) : "なし";
}

interface PositionTableProps {
  positions: Position[];
  onClosePosition: (id: string) => void;
}

export function PositionTable({ positions, onClosePosition }: PositionTableProps) {
  const now = useNow();
  const [signalId, setSignalId] = React.useState<string | null>(null);

  return (
    <Card className="shadow-xs">
      <CardHeader className="pb-3">
        <CardTitle className="text-base font-semibold flex items-center gap-2">
          <Wallet className="h-5 w-5 text-primary" />
          <span>保有ポジション</span>
          <Badge variant="secondary" className="font-mono text-xs">
            {positions.length} 件
          </Badge>
        </CardTitle>
        <CardDescription className="text-xs">
          SL/TP はブローカー側に置いてあり、戦略は自分の時間足の確定ごとに SL の変更・決済を判断します。手動/取込の建玉は戦略が触りません
        </CardDescription>
      </CardHeader>
      <CardContent className="p-0">
        <div className="overflow-x-auto">
          <Table>
            <TableHeader className="bg-muted/40">
              <TableRow>
                <TableHead>銘柄</TableHead>
                <TableHead>戦略 / メモ</TableHead>
                <TableHead className="text-right">建値 → 現在値</TableHead>
                <TableHead className="text-right">SL / TP</TableHead>
                <TableHead className="text-right">損益</TableHead>
                <TableHead className="text-right w-[76px]">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {positions.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="h-20 text-center text-muted-foreground text-sm">
                    保有中のポジションはありません
                  </TableCell>
                </TableRow>
              ) : (
                positions.map((pos) => {
                  const d = priceDigits(pos.symbol);
                  const pnlClass = pos.pnlPips >= 0 ? "text-profit" : "text-loss";
                  const opened = parseEngineTime(pos.openTime);
                  return (
                    <TableRow key={pos.id} className="hover:bg-muted/30">
                      <TableCell>
                        <div className="flex items-center gap-2">
                          <span className="font-semibold text-sm">{pos.symbol}</span>
                          <SideBadge side={pos.side} />
                        </div>
                        <span className="text-[11px] font-mono text-muted-foreground" title={`建玉 ${pos.openTime} (UTC)`}>
                          {pos.volumeLots.toFixed(2)} lot · 保有 {opened != null ? formatDuration(now - opened) : "-"}
                        </span>
                      </TableCell>
                      <TableCell className="max-w-[260px]">
                        <StrategyBadge strategy={pos.strategy} />
                        <p className="mt-0.5 text-[11px] text-muted-foreground truncate" title={pos.note}>
                          {pos.note || "—"}
                        </p>
                      </TableCell>
                      <TableCell className="text-right font-mono tabular-nums text-sm">
                        <span className="text-muted-foreground">{pos.entryPrice.toFixed(d)}</span>
                        <span className="text-muted-foreground"> → </span>
                        <span className="font-semibold">{pos.currentPrice.toFixed(d)}</span>
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex flex-col items-end gap-0.5 font-mono tabular-nums text-xs">
                          <span className="text-loss">SL {formatLevel(pos.stopLoss, d)}</span>
                          <span className="text-profit">TP {formatLevel(pos.takeProfit, d)}</span>
                        </div>
                      </TableCell>
                      <TableCell className={cn("text-right font-mono tabular-nums", pnlClass)}>
                        <div className="text-sm font-semibold">
                          {pos.pnlPips >= 0 ? "+" : ""}
                          {pos.pnlPips.toFixed(1)} pips
                        </div>
                        <div className="text-xs">{yen.format(pos.pnlAmount)}</div>
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex items-center justify-end gap-1">
                          {isSignalId(pos.signalId) && (
                            <Button
                              variant="ghost"
                              size="icon-sm"
                              onClick={() => setSignalId(pos.signalId ?? null)}
                              aria-label="この建玉を建てたシグナル"
                              title="この建玉を建てたシグナル"
                            >
                              <Radio className="h-3.5 w-3.5" />
                            </Button>
                          )}
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            onClick={() => onClosePosition(pos.id)}
                            className="text-loss hover:text-loss hover:bg-loss/10"
                            aria-label="このポジションを成行決済"
                            title="このポジションを成行決済"
                          >
                            <XCircle className="h-3.5 w-3.5" />
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  );
                })
              )}
            </TableBody>
          </Table>
        </div>
      </CardContent>
      <SignalDetailDialog signalId={signalId} onClose={() => setSignalId(null)} />
    </Card>
  );
}
