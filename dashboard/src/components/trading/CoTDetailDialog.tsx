"use client";

import * as React from "react";
import { ActionType, CoTDetail } from "@/types/trading";
import { tradingApi } from "@/lib/trading-api";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
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
  ShieldCheck,
  Target,
  Sparkles,
  TrendingUp,
  Receipt,
} from "lucide-react";
import { cn } from "@/lib/utils";

export function ActionBadge({ action }: { action: ActionType }) {
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
}

interface CoTDetailDialogProps {
  /** 表示する判断の ID。null で閉じる */
  cotLogId: string | null;
  onClose: () => void;
}

/** 判断の根拠と、その判断から建てたポジション・決済結果を表示する */
export function CoTDetailDialog({ cotLogId, onClose }: CoTDetailDialogProps) {
  // 取得結果は ID ごとに持ち、別の判断を開いた直後に前の内容を出さない
  const [loaded, setLoaded] = React.useState<{ id: string; detail?: CoTDetail; error?: string } | null>(null);

  React.useEffect(() => {
    if (!cotLogId) return;
    let cancelled = false;
    tradingApi
      .getCoTDetail(cotLogId)
      .then((res) => {
        if (cancelled) return;
        if (res.success && res.data) setLoaded({ id: cotLogId, detail: res.data });
        else setLoaded({ id: cotLogId, error: res.message ?? "判断ログを取得できませんでした" });
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setLoaded({ id: cotLogId, error: err instanceof Error ? err.message : "判断ログを取得できませんでした" });
      });
    return () => {
      cancelled = true;
    };
  }, [cotLogId]);

  const current = loaded?.id === cotLogId ? loaded : null;
  const detail = current?.detail;
  const error = current?.error;
  const log = detail?.log;
  const digits = log?.symbol.includes("JPY") ? 3 : 5;

  return (
    <Dialog open={!!cotLogId} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-2xl max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <div className="flex items-center justify-between pr-6">
            <DialogTitle className="flex items-center gap-2.5 text-lg">
              <BrainCircuit className="h-5 w-5 text-indigo-500" />
              <span>推論詳細{log ? `: ${log.symbol}` : ""}</span>
              {log && <ActionBadge action={log.action} />}
            </DialogTitle>
          </div>
          <DialogDescription className="text-xs font-mono text-muted-foreground">
            推論ID: {cotLogId}
            {log && ` | 時刻: ${log.timestamp} | スプレッド: ${log.spreadPips} pips`}
          </DialogDescription>
        </DialogHeader>

        {error ? (
          <p className="text-sm text-destructive py-4">{error}</p>
        ) : !detail || !log ? (
          <div className="space-y-3 py-2">
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-24 w-full" />
            <Skeleton className="h-24 w-full" />
          </div>
        ) : (
          <div className="space-y-4 py-2">
            {/* 確信度 & リスクリワード */}
            <div className="grid grid-cols-2 sm:grid-cols-3 gap-3 p-3 rounded-lg bg-muted/40 border">
              <div>
                <span className="text-xs text-muted-foreground">推論確信度</span>
                <div className="text-lg font-bold font-mono text-indigo-600 dark:text-indigo-400">
                  {(log.confidence * 100).toFixed(0)}%
                </div>
              </div>

              {log.action !== "HOLD" && (
                <>
                  <div>
                    <span className="text-xs text-muted-foreground">想定リスクリワード比</span>
                    <div className="text-lg font-bold font-mono text-emerald-600 dark:text-emerald-400">
                      1 : {log.riskRewardRatio?.toFixed(2)}
                    </div>
                  </div>
                  <div>
                    <span className="text-xs text-muted-foreground">発注執行ステータス</span>
                    <div className="text-sm font-semibold mt-1">
                      {log.executed ? (
                        <span className="text-emerald-600 dark:text-emerald-400 flex items-center gap-1">
                          <ShieldCheck className="h-4 w-4" />
                          発注済み
                        </span>
                      ) : (
                        <span className="text-muted-foreground">
                          未発注{log.guardResult ? `（${log.guardResult}）` : ""}
                        </span>
                      )}
                    </div>
                  </div>
                </>
              )}
            </div>

            {/* この判断で行った取引 */}
            <div className="space-y-2 p-3 rounded-lg border bg-card">
              <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                <Receipt className="h-4 w-4" />
                <span>この判断で行った取引</span>
              </div>
              {detail.openPositions.length === 0 && detail.trades.length === 0 ? (
                <p className="text-sm text-muted-foreground">この判断に紐づく取引はありません</p>
              ) : (
                <ul className="space-y-1.5 text-sm">
                  {detail.openPositions.map((p) => (
                    <li key={p.id} className="flex flex-wrap items-center justify-between gap-2">
                      <span className="flex items-center gap-2">
                        <Badge variant="outline" className="text-[11px]">保有中</Badge>
                        <span className="font-mono">
                          {p.side} {p.volumeLots.toFixed(2)} lot @ {p.entryPrice.toFixed(digits)}
                        </span>
                      </span>
                      <span className="text-xs text-muted-foreground font-mono">{p.openTime}</span>
                    </li>
                  ))}
                  {detail.trades.map((t) => (
                    <li key={t.id} className="flex flex-wrap items-center justify-between gap-2">
                      <span className="flex items-center gap-2">
                        <Badge variant="secondary" className="text-[11px]">決済済み</Badge>
                        <span className="font-mono">
                          {t.side} {t.volumeLots.toFixed(2)} lot {t.entryPrice.toFixed(digits)} → {t.closePrice.toFixed(digits)}
                        </span>
                        <span
                          className={cn(
                            "font-mono font-semibold",
                            t.pnlPips >= 0
                              ? "text-emerald-600 dark:text-emerald-400"
                              : "text-rose-600 dark:text-rose-400"
                          )}
                        >
                          {t.pnlPips >= 0 ? "+" : ""}
                          {t.pnlPips.toFixed(1)} pips
                        </span>
                      </span>
                      <span className="text-xs text-muted-foreground font-mono">{t.closeTime}</span>
                    </li>
                  ))}
                </ul>
              )}
            </div>

            {/* 価格設定 (BUY / SELLの場合) */}
            {log.action !== "HOLD" && log.entryPrice && (
              <div className="grid grid-cols-3 gap-2 text-center p-3 rounded-lg border bg-card">
                <div>
                  <span className="text-xs text-muted-foreground">想定エントリー価格</span>
                  <p className="font-mono text-base font-bold">{log.entryPrice.toFixed(digits)}</p>
                </div>
                <div>
                  <span className="text-xs text-muted-foreground">ストップロス (SL)</span>
                  <p className="font-mono text-base font-bold text-rose-600 dark:text-rose-400">
                    {log.stopLoss?.toFixed(digits)}
                  </p>
                </div>
                <div>
                  <span className="text-xs text-muted-foreground">テイクプロフィット (TP)</span>
                  <p className="font-mono text-base font-bold text-emerald-600 dark:text-emerald-400">
                    {log.takeProfit?.toFixed(digits)}
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
              <p className="text-sm leading-relaxed">{log.macroContext}</p>
            </div>

            {/* 注文の攻防 (15M / 5M) */}
            <div className="space-y-1.5 p-3 rounded-lg border bg-card">
              <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                <Sparkles className="h-4 w-4 text-amber-500" />
                <span>2. 注文の攻防 (15M / 5M)</span>
              </div>
              <p className="text-sm leading-relaxed font-medium">{log.orderFlow}</p>
            </div>

            {/* シナリオ無効化価格 (客観的SL理由) */}
            <div className="space-y-1.5 p-3 rounded-lg border border-rose-500/20 bg-rose-50/30 dark:bg-rose-950/20">
              <div className="flex items-center gap-2 text-xs font-semibold text-rose-600 dark:text-rose-400">
                <ShieldCheck className="h-4 w-4" />
                <span>3. シナリオ無効化ライン（客観的損切り根拠）</span>
              </div>
              <p className="text-sm leading-relaxed text-foreground">{log.invalidation}</p>
            </div>

            {/* 反対材料 */}
            {log.conflicts && (
              <div className="space-y-1.5 p-3 rounded-lg border bg-card">
                <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                  <span>4. 反対材料</span>
                </div>
                <p className="text-sm leading-relaxed">{log.conflicts}</p>
              </div>
            )}

            {/* 総合思考ロジック (Reasoning全文) */}
            <div className="space-y-1.5 p-3 rounded-lg border bg-muted/30">
              <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                <Target className="h-4 w-4 text-indigo-500" />
                <span>5. LLM 総合判断理由 (CoT Reasoning)</span>
              </div>
              <p className="text-sm leading-relaxed text-muted-foreground whitespace-pre-wrap">
                {log.reasoning}
              </p>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
