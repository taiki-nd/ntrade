"use client";

import Link from "next/link";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { buttonVariants } from "@/components/ui/button";
import { Bot, Settings } from "lucide-react";
import { cn } from "@/lib/utils";
import { formatParams, TIMEFRAME_LABELS } from "@/lib/strategy";
import type { DeploymentView } from "@/types/trading";

interface DeploymentStatusCardProps {
  /** null の間は読み込み中 */
  deployments: DeploymentView[] | null;
}

/** 稼働中（設定済み）の戦略と、直近の実行状況 */
export function DeploymentStatusCard({ deployments }: DeploymentStatusCardProps) {
  const live = deployments?.filter((d) => d.enabled && !d.dryRun).length ?? 0;

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-row items-start justify-between gap-4 pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <Bot className="h-5 w-5 text-primary" />
            <span>稼働中の戦略</span>
            {deployments && (
              <Badge variant="secondary" className="font-mono text-xs">
                発注 {live} / {deployments.length}
              </Badge>
            )}
          </CardTitle>
          <CardDescription className="text-xs">
            各戦略は自分の時間足の確定ごとに動きます。発注するのは「有効」かつドライランでないものだけです
          </CardDescription>
        </div>
        <Link href="/settings" className={cn(buttonVariants({ variant: "outline", size: "sm" }), "h-8 gap-1.5 text-xs shrink-0")}>
          <Settings className="h-3.5 w-3.5" />
          デプロイを編集
        </Link>
      </CardHeader>
      <CardContent className="p-0">
        <div className="overflow-x-auto">
          <Table>
            <TableHeader className="bg-muted/40">
              <TableRow>
                <TableHead>デプロイ</TableHead>
                <TableHead>戦略</TableHead>
                <TableHead>状態</TableHead>
                <TableHead className="text-right">建玉 / 注文</TableHead>
                <TableHead>直近の足 (UTC)</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {deployments === null ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-16 text-center text-muted-foreground text-sm">
                    読み込み中...
                  </TableCell>
                </TableRow>
              ) : deployments.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-16 text-center text-muted-foreground text-sm">
                    デプロイがありません。システム設定の「戦略のデプロイ」から追加してください
                  </TableCell>
                </TableRow>
              ) : (
                deployments.map((d) => (
                  <TableRow key={d.id} className="hover:bg-muted/30">
                    <TableCell>
                      <div className="font-mono text-sm font-semibold">{d.id}</div>
                      <div className="text-[11px] text-muted-foreground">{d.pair}</div>
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-1.5">
                        <span className="font-mono text-xs">{d.strategy.name}</span>
                        <Badge variant="outline" className="font-mono text-[10px]">
                          {TIMEFRAME_LABELS[d.strategy.timeframe] ?? d.strategy.timeframe}
                        </Badge>
                      </div>
                      <div className="text-[11px] font-mono text-muted-foreground">{formatParams(d.strategy.params) || "既定パラメータ"}</div>
                    </TableCell>
                    <TableCell>
                      <div className="flex flex-wrap gap-1">
                        <Badge
                          variant="outline"
                          className={cn("text-[11px]", d.enabled ? "border-profit/40 text-profit" : "text-muted-foreground")}
                        >
                          {d.enabled ? "有効" : "停止"}
                        </Badge>
                        {d.dryRun ? (
                          <Badge variant="outline" className="text-[11px] text-muted-foreground" title="発注せずシグナルを記録するだけ">
                            ドライラン
                          </Badge>
                        ) : (
                          <Badge variant="outline" className="text-[11px] border-primary/40 text-primary">
                            本番発注
                          </Badge>
                        )}
                      </div>
                    </TableCell>
                    <TableCell className="text-right font-mono tabular-nums text-sm">
                      {d.openPositions} / {d.workingOrders}
                    </TableCell>
                    <TableCell className="max-w-[280px]">
                      <div className="font-mono text-xs" title={d.status?.lastRun ? `判断 ${d.status.lastRun}` : undefined}>
                        {d.status?.lastBar ?? "—"}
                        {d.status && d.status.lastActions > 0 && (
                          <span className="text-muted-foreground"> · 操作 {d.status.lastActions}</span>
                        )}
                      </div>
                      {d.status?.lastError && (
                        <p className="text-[11px] text-loss truncate" title={d.status.lastError}>
                          {d.status.lastError}
                        </p>
                      )}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>
  );
}
