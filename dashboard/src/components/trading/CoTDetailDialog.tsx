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
  ShieldAlert,
} from "lucide-react";
import { cn } from "@/lib/utils";

export function ActionBadge({ action, className }: { action: ActionType; className?: string }) {
  switch (action) {
    case "BUY":
      return (
        <Badge className={cn("bg-emerald-600 hover:bg-emerald-600 text-white gap-1 shrink-0", className)}>
          <ArrowUpRight className="h-3.5 w-3.5" />
          BUY
        </Badge>
      );
    case "SELL":
      return (
        <Badge className={cn("bg-rose-600 hover:bg-rose-600 text-white gap-1 shrink-0", className)}>
          <ArrowDownRight className="h-3.5 w-3.5" />
          SELL
        </Badge>
      );
    case "HOLD":
      return (
        <Badge variant="outline" className={cn("text-muted-foreground gap-1 border-dashed shrink-0", className)}>
          <MinusCircle className="h-3.5 w-3.5" />
          HOLD (見送り)
        </Badge>
      );
  }
}

export function getGuardShortLabel(guardResult?: string): string | null {
  if (!guardResult || guardResult === "PASS") return null;
  const parts = guardResult.split("|").map((p) => p.trim()).filter(Boolean);
  if (parts.length === 0) return null;

  const labelMap: Record<string, string> = {
    CONFIDENCE_LOW: "確信度不足",
    RR_TOO_LOW: "RR比不足",
    SL_TOO_TIGHT: "SL狭小",
    SL_ATR_RANGE: "ATR不一致",
    SL_WRONG_SIDE: "SL方向不正",
    TP_WRONG_SIDE: "TP方向不正",
    NO_SL_TP: "SL/TP未設定",
    SPREAD_TOO_WIDE: "スプレッド大",
    MAX_POSITIONS: "上限到達",
    DAILY_LOSS_LIMIT: "損失限度到達",
    NEWS_BLACKOUT: "指標前後",
    OBSERVED_MISMATCH: "観測不整合",
    PLAN_UNSTRUCTURED: "プラン不正",
    PLAN_NO_SL_TP: "プランSL未設定",
    PLAN_EXPIRY: "期限切れ",
    PLAN_SL_WRONG_SIDE: "プランSL不正",
    PLAN_TP_WRONG_SIDE: "プランTP不正",
  };

  const firstLabel = labelMap[parts[0]] ?? "ガード抑止";
  if (parts.length > 1) {
    return `${firstLabel} 他`;
  }
  return firstLabel;
}

export function ExecutionBadge({
  executed,
  action,
  guardResult,
  className,
}: {
  executed: boolean;
  action: ActionType;
  guardResult?: string;
  className?: string;
}) {
  if (action === "HOLD") return null;

  if (executed) {
    return (
      <Badge
        variant="outline"
        className={cn(
          "text-[10px] text-emerald-600 dark:text-emerald-400 border-emerald-500/30 bg-emerald-500/10 gap-1 shrink-0 py-0 px-1.5 h-5 font-medium",
          className
        )}
      >
        <ShieldCheck className="h-3 w-3" />
        発注済
      </Badge>
    );
  }

  const shortLabel = getGuardShortLabel(guardResult);

  return (
    <Badge
      variant="outline"
      className={cn(
        "text-[10px] text-amber-700 dark:text-amber-400 border-amber-500/40 bg-amber-500/10 gap-1 shrink-0 py-0 px-1.5 h-5 font-semibold",
        className
      )}
      title={guardResult ? `未発注理由: ${guardResult}` : "未発注"}
    >
      <ShieldAlert className="h-3 w-3" />
      未発注{shortLabel ? ` (${shortLabel})` : ""}
    </Badge>
  );
}

interface GuardReasonInfo {
  code: string;
  title: string;
  description: string;
}

export function getEffectiveRiskRewardRatio(log?: CoTDetail["log"]): number | null {
  if (!log) return null;
  if (log.riskRewardRatio != null) return log.riskRewardRatio;
  if (
    log.entryPrice != null &&
    log.stopLoss != null &&
    log.takeProfit != null &&
    Math.abs(log.entryPrice - log.stopLoss) > 0
  ) {
    return Math.abs((log.takeProfit - log.entryPrice) / (log.entryPrice - log.stopLoss));
  }
  return null;
}

function getGuardReasonDetails(code: string, log?: CoTDetail["log"]): GuardReasonInfo {
  const trimmed = code.trim();
  switch (trimmed) {
    case "CONFIDENCE_LOW":
      return {
        code: trimmed,
        title: "推論確信度不足",
        description: log
          ? `推論確信度（${(log.confidence * 100).toFixed(0)}%）が発注基準（70%）を下回っているため、安全のため見送りました。`
          : "推論確信度が発注基準（70%）を下回っているため、安全のため見送りました。",
      };
    case "RR_TOO_LOW": {
      const rr = getEffectiveRiskRewardRatio(log);
      return {
        code: trimmed,
        title: "想定リスクリワード比不足",
        description: rr != null
          ? `想定リスクリワード比（1 : ${rr.toFixed(2)}）が最低基準（1 : 1.5）未満のため、十分な期待値が得られないと判断されました。`
          : "想定リスクリワード比が最低基準（1 : 1.5）未満のため、十分な期待値が得られないと判断されました。",
      };
    }
    case "SL_TOO_TIGHT":
      return {
        code: trimmed,
        title: "損切り幅が狭すぎる",
        description: "ストップロスまでの距離が近すぎるため、ノイズによる即時ロスカットを避けるため見送りました。",
      };
    case "SL_ATR_RANGE":
      return {
        code: trimmed,
        title: "損切り幅のボラティリティ不一致",
        description: "損切り幅が直近のボラティリティ（ATR）の適正範囲外（過小または過大）です。",
      };
    case "SL_WRONG_SIDE":
      return {
        code: trimmed,
        title: "損切り価格の方向不正",
        description: "注文方向（買い/売り）に対して損切り（SL）価格の設定方向が矛盾しています。",
      };
    case "TP_WRONG_SIDE":
      return {
        code: trimmed,
        title: "利確価格の方向不正",
        description: "注文方向（買い/売り）に対して利確（TP）価格の設定方向が矛盾しています。",
      };
    case "NO_SL_TP":
      return {
        code: trimmed,
        title: "損切り/利確の未設定",
        description: "エントリー価格、ストップロス、テイクプロフィットのいずれかが設定されていません。",
      };
    case "SPREAD_TOO_WIDE":
      return {
        code: trimmed,
        title: "スプレッド拡大",
        description: log
          ? `スプレッド（${log.spreadPips} pips）が許容最大値を超えて拡大しているため、コスト増を回避し見送りました。`
          : "スプレッドが許容最大値を超えて拡大しているため、コスト増を回避し見送りました。",
      };
    case "MAX_POSITIONS":
      return {
        code: trimmed,
        title: "最大ポジション数上限",
        description: "該当通貨ペアまたは口座全体の同時保有ポジション上限に達しているため、新規発注を見送りました。",
      };
    case "DAILY_LOSS_LIMIT":
      return {
        code: trimmed,
        title: "当日損失限度到達",
        description: "当日の確定損失が許容上限（日次ドローダウン限度）に達しているため、リスク管理のため新規発注を停止しています。",
      };
    case "NEWS_BLACKOUT":
      return {
        code: trimmed,
        title: "重要指標・ニュース発表前後",
        description: "重要経済指標の発表前後の取引停止時間帯に該当するため、突発的な急変動を避けるため発注を見送りました。",
      };
    case "OBSERVED_MISMATCH":
      return {
        code: trimmed,
        title: "観測足データの不整合",
        description: "LLMが参照した直近ローソク足データとシステムのスナップショットデータに不整合が検出されました。",
      };
    case "PLAN_UNSTRUCTURED":
      return {
        code: trimmed,
        title: "プラン条件の構造不正",
        description: "条件付き注文のトリガー条件または無効化条件が正しく設定されていません。",
      };
    case "PLAN_NO_SL_TP":
      return {
        code: trimmed,
        title: "プラン損切り/利確未設定",
        description: "条件付き注文のストップロスまたはテイクプロフィットが設定されていません。",
      };
    case "PLAN_EXPIRY":
      return {
        code: trimmed,
        title: "プラン有効期限不正",
        description: "条件付き注文の有効期限が切れているか、許容される最大有効期限を超えています。",
      };
    case "PLAN_SL_WRONG_SIDE":
      return {
        code: trimmed,
        title: "プラン損切り価格の方向不正",
        description: "プランの注文方向に対して損切り価格の設定方向が矛盾しています。",
      };
    case "PLAN_TP_WRONG_SIDE":
      return {
        code: trimmed,
        title: "プラン利確価格の方向不正",
        description: "プランの注文方向に対して利確価格の設定方向が矛盾しています。",
      };
    default:
      return {
        code: trimmed,
        title: `事後ガードによる抑止 (${trimmed})`,
        description: "リスク管理ルール（事後ガード）の検証に合格しなかったため、安全のため発注を見送りました。",
      };
  }
}

function parseGuardReasons(guardResult?: string, log?: CoTDetail["log"]): GuardReasonInfo[] {
  if (!guardResult || guardResult === "PASS") return [];
  return guardResult
    .split("|")
    .map((r) => r.trim())
    .filter(Boolean)
    .map((code) => getGuardReasonDetails(code, log));
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
  const guardReasons = React.useMemo(() => parseGuardReasons(log?.guardResult, log), [log]);
  const effectiveRR = React.useMemo(() => getEffectiveRiskRewardRatio(log), [log]);

  return (
    <Dialog open={!!cotLogId} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-2xl max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <div className="flex items-center justify-between pr-6">
            <DialogTitle className="flex items-center gap-2.5 text-lg flex-wrap">
              <BrainCircuit className="h-5 w-5 text-indigo-500 shrink-0" />
              <span>推論詳細{log ? `: ${log.symbol}` : ""}</span>
              {log && (
                <div className="flex items-center gap-1.5 shrink-0">
                  <ActionBadge action={log.action} />
                  <ExecutionBadge
                    action={log.action}
                    executed={log.executed}
                    guardResult={log.guardResult}
                  />
                </div>
              )}
            </DialogTitle>
          </div>
          <DialogDescription className="text-xs font-mono text-muted-foreground break-all">
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
              <div className="min-w-0">
                <span className="text-xs text-muted-foreground">推論確信度</span>
                <div className="text-lg font-bold font-mono text-indigo-600 dark:text-indigo-400">
                  {(log.confidence * 100).toFixed(0)}%
                </div>
              </div>

              {log.action !== "HOLD" && (
                <>
                  <div className="min-w-0">
                    <span className="text-xs text-muted-foreground">想定リスクリワード比</span>
                    <div className="text-lg font-bold font-mono text-emerald-600 dark:text-emerald-400">
                      {effectiveRR != null ? `1 : ${effectiveRR.toFixed(2)}` : "-"}
                    </div>
                  </div>
                  <div className="min-w-0">
                    <span className="text-xs text-muted-foreground">発注執行ステータス</span>
                    <div className="text-sm font-semibold mt-1">
                      {log.executed ? (
                        <span className="text-emerald-600 dark:text-emerald-400 flex items-center gap-1">
                          <ShieldCheck className="h-4 w-4 shrink-0" />
                          <span>発注済み</span>
                        </span>
                      ) : (
                        <span className="text-amber-600 dark:text-amber-400 flex items-center gap-1">
                          <ShieldAlert className="h-4 w-4 shrink-0" />
                          <span>未発注（ガード抑止）</span>
                        </span>
                      )}
                    </div>
                  </div>
                </>
              )}
            </div>

            {/* 未発注理由カード (未発注の場合) */}
            {log.action !== "HOLD" && !log.executed && (
              <div className="space-y-2.5 p-3.5 rounded-lg border border-amber-500/30 bg-amber-500/5 dark:bg-amber-950/20">
                <div className="flex items-center justify-between gap-2 flex-wrap">
                  <div className="flex items-center gap-2 text-xs font-semibold text-amber-700 dark:text-amber-400">
                    <ShieldAlert className="h-4 w-4 shrink-0" />
                    <span>未発注理由（事後ガードによる抑止）</span>
                  </div>
                  {log.guardResult && (
                    <span className="text-[11px] font-mono text-muted-foreground bg-muted/60 px-1.5 py-0.5 rounded border">
                      {log.guardResult}
                    </span>
                  )}
                </div>

                {guardReasons.length > 0 ? (
                  <div className="space-y-2 pt-0.5">
                    {guardReasons.map((reason) => (
                      <div
                        key={reason.code}
                        className="p-2.5 rounded-md bg-card/80 border border-amber-500/20 space-y-1 text-xs shadow-xs"
                      >
                        <div className="flex items-center gap-2 flex-wrap">
                          <span className="font-semibold text-foreground">
                            {reason.title}
                          </span>
                          <Badge
                            variant="outline"
                            className="text-[10px] font-mono py-0 px-1.5 h-4 text-muted-foreground border-amber-500/30"
                          >
                            {reason.code}
                          </Badge>
                        </div>
                        <p className="text-muted-foreground leading-relaxed">
                          {reason.description}
                        </p>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-xs text-muted-foreground leading-relaxed">
                    事後ガードまたはシステム条件により、安全のため発注が見送られました。
                  </p>
                )}
              </div>
            )}

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
