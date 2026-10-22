"use client";

import * as React from "react";
import { CoTLog } from "@/types/trading";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { BrainCircuit, Eye, Clock, ListChecks, Search, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { priceDigits } from "@/lib/price";
import {
  ActionBadge,
  ExecutionBadge,
  CoTDetailDialog,
  ENTRY_TYPE_LABELS,
  manageNoteLabel,
  splitGuardResult,
} from "@/components/trading/CoTDetailDialog";
import { PaginationBar, PaginationBarProps } from "@/components/trading/PaginationBar";

export interface CoTSearchProps {
  value: string;
  onChange: (value: string) => void;
}

interface CoTViewerProps {
  logs: CoTLog[];
  /** 指定時はカード下部にページ送りを表示（ページングはサーバー側） */
  pagination?: PaginationBarProps;
  /** 指定時はヘッダーに検索欄を表示（絞り込みはサーバー側） */
  search?: CoTSearchProps;
  /** ヘッダー右側に置く操作（「今すぐ判断」など） */
  actions?: React.ReactNode;
}

export function CoTViewer({ logs, pagination, search, actions }: CoTViewerProps) {
  const [selectedId, setSelectedId] = React.useState<string | null>(null);

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <BrainCircuit className="h-5 w-5 text-primary" />
            <span>LLM の判断</span>
          </CardTitle>
          <CardDescription className="text-xs">
            価格の数値から日中の短期取引を読み、予約注文を置くか・既存の注文と建玉をどうするかを決めた記録
          </CardDescription>
        </div>
        {actions}
        {search && (
          <div className="relative w-full sm:w-64 shrink-0">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground pointer-events-none" />
            <Input
              value={search.value}
              onChange={(e) => search.onChange(e.target.value)}
              placeholder="通貨ペア・判断・根拠を検索"
              aria-label="思考ログを検索"
              className="h-8 pl-8 pr-8 text-xs"
            />
            {search.value && (
              <Button
                variant="ghost"
                size="icon"
                aria-label="検索条件をクリア"
                className="absolute right-0.5 top-1/2 -translate-y-1/2 h-7 w-7"
                onClick={() => search.onChange("")}
              >
                <X className="h-3.5 w-3.5" />
              </Button>
            )}
          </div>
        )}
      </CardHeader>
      <CardContent className="space-y-3">
        {logs.length === 0 && (
          <p className="text-sm text-muted-foreground text-center py-6">
            {search?.value ? "条件に一致する判断ログはありません" : "判断ログはまだありません"}
          </p>
        )}
        {logs.map((log) => {
          const confidencePercent = Math.round(log.confidence * 100);
          const manage = splitGuardResult(log.guardResult).manage;
          const order =
            log.action !== "HOLD" && log.entryPrice != null
              ? `${log.entryType ? `${ENTRY_TYPE_LABELS[log.entryType] ?? log.entryType} ` : ""}${log.entryPrice.toFixed(priceDigits(log.symbol))}`
              : null;
          return (
            <div
              key={log.id}
              onClick={() => setSelectedId(log.id)}
              className="flex items-start gap-3 p-3 rounded-lg border bg-card/60 hover:bg-accent/40 cursor-pointer transition-colors"
            >
              <ActionBadge action={log.action} className="mt-0.5" />
              <div className="min-w-0 flex-1 space-y-1">
                <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                  <span className="font-semibold text-sm">{log.symbol}</span>
                  {order && <span className="text-xs font-mono">{order}</span>}
                  <ExecutionBadge action={log.action} executed={log.executed} guardResult={log.guardResult} />
                  {manage.length > 0 && (
                    <Badge
                      variant="outline"
                      className={cn("text-[10px] gap-1 py-0 px-1.5 h-5", manage.some((n) => n.failed) ? "border-loss/40 text-loss" : "text-muted-foreground")}
                      title={manage.map((n) => `${manageNoteLabel(n)}: ${n.result}`).join("\n")}
                    >
                      <ListChecks className="h-3 w-3" />
                      操作 {manage.length}
                    </Badge>
                  )}
                </div>
                <p className="text-xs text-muted-foreground line-clamp-2" title={log.fundamentalView || log.macroContext}>
                  {log.fundamentalView || log.macroContext}
                </p>
                <span className="text-[11px] text-muted-foreground flex items-center gap-1 font-mono">
                  <Clock className="h-3 w-3 shrink-0" />
                  {log.timestamp}
                </span>
              </div>

              <div className="w-20 space-y-1 shrink-0 pt-0.5">
                <div className="flex justify-between text-[11px]">
                  <span className="text-muted-foreground">確信度</span>
                  <span className="font-mono font-semibold">{confidencePercent}%</span>
                </div>
                <Progress
                  value={confidencePercent}
                  className={cn(
                    "h-1.5",
                    confidencePercent >= 70 ? "[&>div]:bg-profit" : confidencePercent >= 50 ? "[&>div]:bg-warning" : "[&>div]:bg-muted-foreground"
                  )}
                />
              </div>

              <Button
                variant="ghost"
                size="icon-sm"
                className="shrink-0"
                aria-label="根拠を開く"
                title="根拠を開く"
                onClick={(e) => {
                  e.stopPropagation();
                  setSelectedId(log.id);
                }}
              >
                <Eye className="h-3.5 w-3.5" />
              </Button>
            </div>
          );
        })}

        {pagination && <PaginationBar {...pagination} />}
      </CardContent>

      <CoTDetailDialog cotLogId={selectedId} onClose={() => setSelectedId(null)} />
    </Card>
  );
}
