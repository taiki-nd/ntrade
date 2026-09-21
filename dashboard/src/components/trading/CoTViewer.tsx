"use client";

import * as React from "react";
import { CoTLog } from "@/types/trading";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { BrainCircuit, Eye, Clock } from "lucide-react";
import { cn } from "@/lib/utils";
import { ActionBadge, CoTDetailDialog } from "@/components/trading/CoTDetailDialog";
import { PaginationBar, PaginationBarProps } from "@/components/trading/PaginationBar";

interface CoTViewerProps {
  logs: CoTLog[];
  /** 指定時はカード下部にページ送りを表示（ページングはサーバー側） */
  pagination?: PaginationBarProps;
}

export function CoTViewer({ logs, pagination }: CoTViewerProps) {
  const [selectedId, setSelectedId] = React.useState<string | null>(null);

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-row items-center justify-between pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <BrainCircuit className="h-5 w-5 text-indigo-500" />
            <span>LLM 思考プロセス (CoT) ログ</span>
          </CardTitle>
          <CardDescription className="text-xs">
            マルチモーダル画像・幾何学特徴量に基づくローカル推論の意思決定根拠
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {logs.length === 0 && (
          <p className="text-sm text-muted-foreground text-center py-6">判断ログはまだありません</p>
        )}
        {logs.map((log) => {
          const confidencePercent = Math.round(log.confidence * 100);
          return (
            <div
              key={log.id}
              onClick={() => setSelectedId(log.id)}
              className="group flex flex-col sm:flex-row sm:items-center justify-between p-3.5 rounded-lg border bg-card/60 hover:bg-accent/40 cursor-pointer transition-colors gap-3 overflow-hidden"
            >
              {/* 左側: アクションと通貨・時間・理由 */}
              <div className="flex items-center gap-3 min-w-0 flex-1">
                <ActionBadge action={log.action} />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-bold text-sm shrink-0">{log.symbol}</span>
                    <span className="text-xs text-muted-foreground flex items-center gap-1 min-w-0 truncate font-mono">
                      <Clock className="h-3 w-3 shrink-0" />
                      <span className="truncate">{log.timestamp}</span>
                    </span>
                  </div>
                  <p className="text-xs text-muted-foreground truncate mt-0.5" title={log.orderFlow}>
                    {log.orderFlow}
                  </p>
                </div>
              </div>

              {/* 右側: 確信度ゲージと詳細ボタン */}
              <div className="flex items-center gap-3 sm:gap-4 shrink-0 justify-between sm:justify-end">
                <div className="w-24 sm:w-28 space-y-1 shrink-0">
                  <div className="flex justify-between text-xs">
                    <span className="text-muted-foreground text-[11px]">確信度</span>
                    <span className="font-mono font-semibold">{confidencePercent}%</span>
                  </div>
                  <Progress
                    value={confidencePercent}
                    className={cn(
                      "h-1.5",
                      confidencePercent >= 80
                        ? "[&>div]:bg-emerald-500"
                        : confidencePercent >= 50
                        ? "[&>div]:bg-amber-500"
                        : "[&>div]:bg-muted-foreground"
                    )}
                  />
                </div>

                <Button
                  variant="ghost"
                  size="sm"
                  className="h-8 gap-1 text-xs shrink-0 px-2.5"
                  onClick={(e) => {
                    e.stopPropagation();
                    setSelectedId(log.id);
                  }}
                >
                  <Eye className="h-3.5 w-3.5" />
                  <span className="hidden xl:inline">根拠を開く</span>
                  <span className="inline xl:hidden">根拠</span>
                </Button>
              </div>
            </div>
          );
        })}

        {pagination && <PaginationBar {...pagination} />}
      </CardContent>

      <CoTDetailDialog cotLogId={selectedId} onClose={() => setSelectedId(null)} />
    </Card>
  );
}
