"use client";

import * as React from "react";
import { CoTLog, ActionType } from "@/types/trading";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  BrainCircuit,
  ArrowUpRight,
  ArrowDownRight,
  MinusCircle,
  Eye,
  ShieldCheck,
  Target,
  Sparkles,
  TrendingUp,
  Clock,
} from "lucide-react";
import { cn } from "@/lib/utils";

interface CoTViewerProps {
  logs: CoTLog[];
}

export function CoTViewer({ logs }: CoTViewerProps) {
  const [selectedLog, setSelectedLog] = React.useState<CoTLog | null>(null);

  const getActionBadge = (action: ActionType) => {
    switch (action) {
      case "BUY":
        return (
          <Badge className="bg-emerald-600 hover:bg-emerald-600 text-white gap-1">
            <ArrowUpRight className="h-3.5 w-3.5" />
            BUY
          </Badge>
        );
      case "SELL":
        return (
          <Badge className="bg-rose-600 hover:bg-rose-600 text-white gap-1">
            <ArrowDownRight className="h-3.5 w-3.5" />
            SELL
          </Badge>
        );
      case "HOLD":
        return (
          <Badge variant="outline" className="text-muted-foreground gap-1 border-dashed">
            <MinusCircle className="h-3.5 w-3.5" />
            HOLD (見送り)
          </Badge>
        );
    }
  };

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
        {logs.map((log) => {
          const confidencePercent = Math.round(log.confidence * 100);
          return (
            <div
              key={log.id}
              onClick={() => setSelectedLog(log)}
              className="flex flex-col sm:flex-row sm:items-center justify-between p-3.5 rounded-lg border bg-card/60 hover:bg-accent/40 cursor-pointer transition-colors gap-3"
            >
              {/* 左側: アクションと通貨・時間 */}
              <div className="flex items-center gap-3">
                {getActionBadge(log.action)}
                <div>
                  <div className="flex items-center gap-2">
                    <span className="font-bold text-sm">{log.symbol}</span>
                    <span className="text-xs text-muted-foreground flex items-center gap-1">
                      <Clock className="h-3 w-3" />
                      {log.timestamp}
                    </span>
                  </div>
                  <p className="text-xs text-muted-foreground line-clamp-1 mt-0.5">
                    {log.orderFlow}
                  </p>
                </div>
              </div>

              {/* 右側: 確信度ゲージと詳細ボタン */}
              <div className="flex items-center gap-4 sm:justify-end">
                <div className="w-28 space-y-1">
                  <div className="flex justify-between text-xs">
                    <span className="text-muted-foreground">確信度</span>
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

                <Button variant="ghost" size="sm" className="h-8 gap-1 text-xs">
                  <Eye className="h-3.5 w-3.5" />
                  根拠を開く
                </Button>
              </div>
            </div>
          );
        })}
      </CardContent>

      {/* 詳細ダイアログモーダル */}
      <Dialog open={!!selectedLog} onOpenChange={(open) => !open && setSelectedLog(null)}>
        {selectedLog && (
          <DialogContent className="sm:max-w-2xl max-h-[85vh] overflow-y-auto">
            <DialogHeader>
              <div className="flex items-center justify-between pr-6">
                <DialogTitle className="flex items-center gap-2.5 text-lg">
                  <BrainCircuit className="h-5 w-5 text-indigo-500" />
                  <span>推論詳細: {selectedLog.symbol}</span>
                  {getActionBadge(selectedLog.action)}
                </DialogTitle>
              </div>
              <DialogDescription className="text-xs font-mono text-muted-foreground">
                推論ID: {selectedLog.id} | 時刻: {selectedLog.timestamp} | スプレッド: {selectedLog.spreadPips} pips
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-4 py-2">
              {/* 確信度 & リスクリワード */}
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-3 p-3 rounded-lg bg-muted/40 border">
                <div>
                  <span className="text-xs text-muted-foreground">推論確信度</span>
                  <div className="text-lg font-bold font-mono text-indigo-600 dark:text-indigo-400">
                    {(selectedLog.confidence * 100).toFixed(0)}%
                  </div>
                </div>

                {selectedLog.action !== "HOLD" && (
                  <>
                    <div>
                      <span className="text-xs text-muted-foreground">想定リスクリワード比</span>
                      <div className="text-lg font-bold font-mono text-emerald-600 dark:text-emerald-400">
                        1 : {selectedLog.riskRewardRatio?.toFixed(2)}
                      </div>
                    </div>
                    <div>
                      <span className="text-xs text-muted-foreground">発注執行ステータス</span>
                      <div className="text-sm font-semibold mt-1">
                        {selectedLog.executed ? (
                          <span className="text-emerald-600 dark:text-emerald-400 flex items-center gap-1">
                            <ShieldCheck className="h-4 w-4" />
                            デモ口座に成行約定
                          </span>
                        ) : (
                          <span className="text-muted-foreground">未発注</span>
                        )}
                      </div>
                    </div>
                  </>
                )}
              </div>

              {/* 価格設定 (BUY / SELLの場合) */}
              {selectedLog.action !== "HOLD" && selectedLog.entryPrice && (
                <div className="grid grid-cols-3 gap-2 text-center p-3 rounded-lg border bg-card">
                  <div>
                    <span className="text-xs text-muted-foreground">想定エントリー価格</span>
                    <p className="font-mono text-base font-bold">
                      {selectedLog.entryPrice.toFixed(selectedLog.symbol.includes("JPY") ? 3 : 5)}
                    </p>
                  </div>
                  <div>
                    <span className="text-xs text-muted-foreground">ストップロス (SL)</span>
                    <p className="font-mono text-base font-bold text-rose-600 dark:text-rose-400">
                      {selectedLog.stopLoss?.toFixed(selectedLog.symbol.includes("JPY") ? 3 : 5)}
                    </p>
                  </div>
                  <div>
                    <span className="text-xs text-muted-foreground">テイクプロフィット (TP)</span>
                    <p className="font-mono text-base font-bold text-emerald-600 dark:text-emerald-400">
                      {selectedLog.takeProfit?.toFixed(selectedLog.symbol.includes("JPY") ? 3 : 5)}
                    </p>
                  </div>
                </div>
              )}

              {/* 上位足環境認識 (4H / 1H) */}
              <div className="space-y-1.5 p-3 rounded-lg border bg-card">
                <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                  <TrendingUp className="h-4 w-4 text-blue-500" />
                  <span>1. 環境認識 (4H / 1H)</span>
                </div>
                <p className="text-sm leading-relaxed">{selectedLog.macroContext}</p>
              </div>

              {/* 注文の攻防 (15M / 5M) */}
              <div className="space-y-1.5 p-3 rounded-lg border bg-card">
                <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                  <Sparkles className="h-4 w-4 text-amber-500" />
                  <span>2. 注文の攻防 (15M / 5M)</span>
                </div>
                <p className="text-sm leading-relaxed font-medium">
                  {selectedLog.orderFlow}
                </p>
              </div>

              {/* 反対材料 */}
              {selectedLog.conflicts && (
                <div className="space-y-1.5 p-3 rounded-lg border bg-card">
                  <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                    <span>4. 反対材料</span>
                  </div>
                  <p className="text-sm leading-relaxed">{selectedLog.conflicts}</p>
                </div>
              )}

              {/* シナリオ無効化価格 (客観的SL理由) */}
              <div className="space-y-1.5 p-3 rounded-lg border border-rose-500/20 bg-rose-50/30 dark:bg-rose-950/20">
                <div className="flex items-center gap-2 text-xs font-semibold text-rose-600 dark:text-rose-400">
                  <ShieldCheck className="h-4 w-4" />
                  <span>3. シナリオ無効化ライン（客観的損切り根拠）</span>
                </div>
                <p className="text-sm leading-relaxed text-foreground">
                  {selectedLog.invalidation}
                </p>
              </div>

              {/* 総合思考ロジック (Reasoning全文) */}
              <div className="space-y-1.5 p-3 rounded-lg border bg-muted/30">
                <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                  <Target className="h-4 w-4 text-indigo-500" />
                  <span>4. LLM 総合判断理由 (CoT Reasoning)</span>
                </div>
                <p className="text-sm leading-relaxed text-muted-foreground whitespace-pre-wrap">
                  {selectedLog.reasoning}
                </p>
              </div>
            </div>
          </DialogContent>
        )}
      </Dialog>
    </Card>
  );
}
