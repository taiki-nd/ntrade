"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Hourglass, Radio, Trash2 } from "lucide-react";
import { tradingApi, type WorkingOrder } from "@/lib/trading-api";
import { priceDigits } from "@/lib/price";
import { cn } from "@/lib/utils";
import { isSignalId } from "@/lib/strategy";
import { toast } from "sonner";
import { SideBadge, StrategyBadge, formatLevel } from "@/components/trading/PositionTable";
import { SignalDetailDialog } from "@/components/trading/SignalViewer";
import { useNow } from "@/hooks/use-now";
import { formatDuration, parseEngineTime } from "@/lib/time";

interface OrderMonitorProps {
  /** 取消などで一覧が変わったときに親の同期を促す */
  onChanged?: () => void;
  /** 親から再取得を促すための値（変わるたびに取り直す） */
  refreshKey?: number;
}

const KIND_LABELS: Record<WorkingOrder["kind"], string> = { LIMIT: "指値", STOP: "逆指値" };

/** 置いた価格からの RR（SL か TP が未設定なら出さない） */
function riskReward(o: WorkingOrder): number | null {
  if (o.stopLoss <= 0 || o.takeProfit <= 0) return null;
  const risk = Math.abs(o.entryPrice - o.stopLoss);
  return risk > 0 ? Math.abs(o.takeProfit - o.entryPrice) / risk : null;
}

export function OrderMonitor({ onChanged, refreshKey }: OrderMonitorProps) {
  const [orders, setOrders] = React.useState<WorkingOrder[]>([]);
  const [signalId, setSignalId] = React.useState<string | null>(null);
  const now = useNow();

  const load = React.useCallback(async () => {
    try {
      const res = await tradingApi.getOrders();
      if (res.success && res.data) setOrders(res.data);
    } catch {
      // エンジン未起動時は無視
    }
  }, []);

  React.useEffect(() => {
    const initial = setTimeout(load, 0);
    const interval = setInterval(load, 10000);
    return () => {
      clearTimeout(initial);
      clearInterval(interval);
    };
  }, [load, refreshKey]);

  const handleCancel = async (id: string) => {
    try {
      const res = await tradingApi.cancelOrder(id);
      if (res.success) {
        toast.info("予約注文を取り消しました");
      } else {
        toast.error("取消に失敗しました", { description: res.message });
      }
      await load();
      onChanged?.();
    } catch {
      toast.error("取消に失敗しました");
    }
  };

  return (
    <Card className="shadow-xs">
      <CardHeader className="pb-3">
        <CardTitle className="text-base font-semibold flex items-center gap-2">
          <Hourglass className="h-5 w-5 text-primary" />
          <span>予約注文</span>
          <Badge variant="secondary" className="font-mono text-xs">
            {orders.length} 件
          </Badge>
        </CardTitle>
        <CardDescription className="text-xs">
          戦略がブローカーに置いた指値・逆指値。約定はブローカーが判定し、戦略は自分の時間足の確定ごとに変更・取消を判断します
        </CardDescription>
      </CardHeader>
      <CardContent className="p-0">
        <div className="overflow-x-auto">
          <Table>
            <TableHeader className="bg-muted/40">
              <TableRow>
                <TableHead>銘柄</TableHead>
                <TableHead>戦略 / メモ</TableHead>
                <TableHead className="text-right">注文価格</TableHead>
                <TableHead className="text-right">SL / TP</TableHead>
                <TableHead className="text-right">RR</TableHead>
                <TableHead className="text-right">期限まで</TableHead>
                <TableHead className="text-right w-[76px]">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {orders.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={7} className="h-20 text-center text-muted-foreground text-sm">
                    約定待ちの予約注文はありません
                  </TableCell>
                </TableRow>
              ) : (
                orders.map((o) => {
                  const d = priceDigits(o.symbol);
                  const rr = riskReward(o);
                  const expires = parseEngineTime(o.expiresAt);
                  const left = expires != null ? expires - now : null;
                  return (
                    <TableRow key={o.id} className="hover:bg-muted/30">
                      <TableCell>
                        <div className="flex items-center gap-2">
                          <span className="font-semibold text-sm">{o.symbol}</span>
                          <SideBadge side={o.side} suffix={KIND_LABELS[o.kind]} />
                        </div>
                        <span className="text-[11px] font-mono text-muted-foreground">{o.volumeLots.toFixed(2)} lot</span>
                      </TableCell>
                      <TableCell className="max-w-[260px]">
                        <StrategyBadge strategy={o.strategy} />
                        <p className="mt-0.5 text-[11px] text-muted-foreground truncate" title={o.note}>
                          {o.note || "—"}
                        </p>
                      </TableCell>
                      <TableCell className="text-right font-mono tabular-nums text-sm font-semibold">{o.entryPrice.toFixed(d)}</TableCell>
                      <TableCell className="text-right">
                        <div className="flex flex-col items-end gap-0.5 font-mono tabular-nums text-xs">
                          <span className="text-loss">SL {formatLevel(o.stopLoss, d)}</span>
                          <span className="text-profit">TP {formatLevel(o.takeProfit, d)}</span>
                        </div>
                      </TableCell>
                      <TableCell className="text-right font-mono tabular-nums text-xs">{rr != null ? `1 : ${rr.toFixed(2)}` : "-"}</TableCell>
                      <TableCell
                        className={cn(
                          "text-right font-mono tabular-nums text-xs",
                          left != null && left < 10 * 60000 ? "text-warning" : "text-muted-foreground"
                        )}
                        title={`発注 ${o.createdAt} / 期限 ${o.expiresAt} (UTC)`}
                      >
                        {left == null ? "-" : left <= 0 ? "期限切れ（照合待ち）" : formatDuration(left)}
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex items-center justify-end gap-1">
                          {isSignalId(o.signalId) && (
                            <Button
                              variant="ghost"
                              size="icon-sm"
                              onClick={() => setSignalId(o.signalId ?? null)}
                              aria-label="この注文を置いたシグナル"
                              title="この注文を置いたシグナル"
                            >
                              <Radio className="h-3.5 w-3.5" />
                            </Button>
                          )}
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            className="text-loss hover:text-loss hover:bg-loss/10"
                            onClick={() => handleCancel(o.id)}
                            aria-label="ブローカーで取り消す"
                            title="ブローカーで取り消す"
                          >
                            <Trash2 className="h-3.5 w-3.5" />
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
