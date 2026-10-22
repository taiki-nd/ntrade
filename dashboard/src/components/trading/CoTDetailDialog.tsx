"use client";

import * as React from "react";
import { ActionType, CoTDetail } from "@/types/trading";
import { tradingApi } from "@/lib/trading-api";
import { priceDigits } from "@/lib/price";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
  GitBranch,
  Landmark,
  ListChecks,
} from "lucide-react";
import { cn } from "@/lib/utils";

export function ActionBadge({ action, className }: { action: ActionType; className?: string }) {
  switch (action) {
    case "BUY":
      return (
        <Badge className={cn("bg-profit hover:bg-profit/90 text-white gap-1 shrink-0", className)}>
          <ArrowUpRight className="h-3.5 w-3.5" />
          BUY
        </Badge>
      );
    case "SELL":
      return (
        <Badge className={cn("bg-loss hover:bg-loss/90 text-white gap-1 shrink-0", className)}>
          <ArrowDownRight className="h-3.5 w-3.5" />
          SELL
        </Badge>
      );
    case "HOLD":
      return (
        <Badge variant="outline" className={cn("text-muted-foreground gap-1 border-dashed shrink-0", className)}>
          <MinusCircle className="h-3.5 w-3.5" />
          HOLD
        </Badge>
      );
  }
}

/** 新規注文の種類（旧データの成行・条件付きプランを含む） */
export const ENTRY_TYPE_LABELS: Record<string, string> = {
  LIMIT: "指値",
  STOP: "逆指値",
  MARKET: "成行",
  CONDITIONAL: "条件付き",
};

/** 判断で行った、既存の予約注文・建玉への操作 1 件 */
export interface ManageNote {
  op: string;
  target: string;
  id: string;
  result: string;
  failed: boolean;
}

const MANAGE_OP_LABELS: Record<string, string> = { CANCEL: "取消", CLOSE: "決済", MODIFY: "変更" };
const MANAGE_TARGET_LABELS: Record<string, string> = { ORDER: "予約注文", POSITION: "建玉" };

/**
 * 判断ログの guard_result を「新規注文の不通過理由・発注エラー」と「注文・建玉への操作」に分ける。
 * エンジンは `<ガード名|...> / MANAGE <OP> <TARGET> <ID>: <結果> / MANAGE ...` の形で記録する
 */
export function splitGuardResult(guardResult?: string): { guard?: string; manage: ManageNote[] } {
  if (!guardResult) return { manage: [] };
  const chunks = guardResult.startsWith("MANAGE ") ? ["", guardResult.slice(7)] : guardResult.split(" / MANAGE ");
  const guard = chunks[0].trim() || undefined;
  const manage = chunks.slice(1).map((c) => {
    const m = c.replace(/^MANAGE /, "").match(/^(\S+) (\S+) (\S+): ([\s\S]*)$/);
    if (!m) return { op: "", target: "", id: "", result: c, failed: c.includes("FAILED") };
    return { op: m[1], target: m[2], id: m[3], result: m[4], failed: m[4].startsWith("FAILED") };
  });
  return { guard: guard === "PASS" ? undefined : guard, manage };
}

/** 操作 1 件の表示用ラベル（例: 「建玉 123 を変更」） */
export function manageNoteLabel(n: ManageNote): string {
  if (!n.op) return n.result;
  return `${MANAGE_TARGET_LABELS[n.target] ?? n.target} ${n.id} を${MANAGE_OP_LABELS[n.op] ?? n.op}`;
}

export function getGuardShortLabel(guardResult?: string): string | null {
  const guard = splitGuardResult(guardResult).guard;
  if (!guard) return null;
  const parts = guard.split("|").map((p) => p.trim()).filter(Boolean);
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
    ENTRY_TYPE: "注文種別不正",
    ORDER_PRICE_SIDE: "注文価格の位置不正",
    ORDER_EXPIRY: "期限不正",
    PLAN_UNSTRUCTURED: "プラン不正",
    PLAN_NO_SL_TP: "プランSL未設定",
    PLAN_EXPIRY: "期限切れ",
    PLAN_SL_WRONG_SIDE: "プランSL不正",
    PLAN_TP_WRONG_SIDE: "プランTP不正",
    ORDER_FAILED: "発注エラー",
    MARGIN_LIMIT: "証拠金上限",
    MARGIN_UNKNOWN: "証拠金不明",
    LEVERAGE_LIMIT: "レバレッジ上限",
    VOLUME_BELOW_MIN: "最小ロット未満",
  };

  const rawFirst = parts[0];
  const baseKey = rawFirst.includes(":") ? rawFirst.slice(0, rawFirst.indexOf(":")).trim() : rawFirst;
  const firstLabel = labelMap[baseKey] ?? (baseKey.startsWith("ORDER_FAILED") ? "発注エラー" : "ガード抑止");
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
          "text-[10px] text-profit border-profit/30 bg-profit/10 gap-1 shrink-0 py-0 px-1.5 h-5 font-medium",
          className
        )}
      >
        <ShieldCheck className="h-3 w-3" />
        注文済
      </Badge>
    );
  }

  const isOrderFailed = splitGuardResult(guardResult).guard?.includes("ORDER_FAILED");
  const shortLabel = getGuardShortLabel(guardResult);

  if (isOrderFailed) {
    return (
      <Badge
        variant="outline"
        className={cn(
          "text-[10px] text-loss border-loss/40 bg-loss/10 gap-1 shrink-0 py-0 px-1.5 h-5 font-semibold",
          className
        )}
        title={guardResult ? `発注エラー: ${guardResult}` : "発注エラー"}
      >
        <ShieldAlert className="h-3 w-3" />
        未発注 (発注エラー)
      </Badge>
    );
  }

  return (
    <Badge
      variant="outline"
      className={cn(
        "text-[10px] text-warning border-warning/40 bg-warning/10 gap-1 shrink-0 py-0 px-1.5 h-5 font-semibold",
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
  const colonIndex = trimmed.indexOf(":");
  const key = colonIndex !== -1 ? trimmed.slice(0, colonIndex).trim() : trimmed;
  const extraDetail = colonIndex !== -1 ? trimmed.slice(colonIndex + 1).trim() : null;

  if (key === "ORDER_FAILED" || key.startsWith("ORDER_FAILED")) {
    return {
      code: "ORDER_FAILED",
      title: "注文執行エラー",
      description: extraDetail
        ? `事後ガードは通過しましたが、注文執行時にエラーが発生しました: ${extraDetail}`
        : "事後ガードは通過しましたが、ブローカーへの発注処理でエラーが発生しました。",
    };
  }

  switch (key) {
    case "ENTRY_TYPE":
      return {
        code: key,
        title: "注文の種類が不正",
        description: "新規の注文は指値（LIMIT）か逆指値（STOP）だけです。成行や種類の無い注文は出しません。",
      };
    case "ORDER_PRICE_SIDE":
      return {
        code: key,
        title: "注文価格の位置が種類と合わない",
        description: extraDetail
          ? `指値は現在値より有利な側、逆指値は不利な側に置く必要があります（${extraDetail}）。逆側に置くと即座に約定するかブローカーに拒否されます。`
          : "指値は現在値より有利な側、逆指値は不利な側に置く必要があります。",
      };
    case "ORDER_EXPIRY":
      return {
        code: key,
        title: "注文の期限が不正",
        description: extraDetail
          ? `予約注文には上限内の有効期限が必要です（${extraDetail}）。`
          : "予約注文には設定の上限内の有効期限が必要です。",
      };
    case "MARGIN_LIMIT":
      return {
        code: key,
        title: "証拠金使用率の上限",
        description: `発注すると証拠金使用率が上限を超えるため見送りました${extraDetail ? `（${extraDetail}）` : ""}。`,
      };
    case "MARGIN_UNKNOWN":
      return {
        code: key,
        title: "必要証拠金を確認できない",
        description: `ブローカーから必要証拠金を取得できなかったため、レバレッジ取引の安全のため見送りました${extraDetail ? `（${extraDetail}）` : ""}。`,
      };
    case "LEVERAGE_LIMIT":
      return {
        code: key,
        title: "実効レバレッジの上限",
        description: `発注すると実効レバレッジが上限を超えるため見送りました${extraDetail ? `（${extraDetail}）` : ""}。`,
      };
    case "VOLUME_BELOW_MIN":
      return {
        code: key,
        title: "最小発注量未満",
        description: `許容リスクから求めたロットがブローカーの最小発注量に届かないため見送りました（最小まで増やすと許容リスクを超えるため）${extraDetail ? `: ${extraDetail}` : ""}。`,
      };
    case "CONFIDENCE_LOW":
      return {
        code: key,
        title: "推論確信度不足",
        description: extraDetail
          ? `推論確信度が発注基準を下回っています（${extraDetail}）。`
          : log
          ? `推論確信度（${(log.confidence * 100).toFixed(0)}%）が発注基準（70%）を下回っているため、安全のため見送りました。`
          : "推論確信度が発注基準（70%）を下回っているため、安全のため見送りました。",
      };
    case "RR_TOO_LOW": {
      const rr = getEffectiveRiskRewardRatio(log);
      return {
        code: key,
        title: "想定リスクリワード比不足",
        description: extraDetail
          ? `想定リスクリワード比が最低基準未満です（${extraDetail}）。`
          : rr != null
          ? `想定リスクリワード比（1 : ${rr.toFixed(2)}）が最低基準（1 : 1.5）未満のため、十分な期待値が得られないと判断されました。`
          : "想定リスクリワード比が最低基準（1 : 1.5）未満のため、十分な期待値が得られないと判断されました。",
      };
    }
    case "SL_TOO_TIGHT":
      return {
        code: key,
        title: "損切り幅が狭すぎる",
        description: extraDetail
          ? `ストップロスまでの距離が近すぎます（${extraDetail}）。ノイズによる即時ロスカットを避けるため見送りました。`
          : "ストップロスまでの距離が近すぎるため、ノイズによる即時ロスカットを避けるため見送りました。",
      };
    case "SL_ATR_RANGE":
      return {
        code: key,
        title: "損切り幅のボラティリティ不一致",
        description: extraDetail
          ? `損切り幅が直近ボラティリティ（ATR）の適正範囲外です（${extraDetail}）。`
          : "損切り幅が直近のボラティリティ（ATR）の適正範囲外（過小または過大）です。",
      };
    case "SL_WRONG_SIDE":
      return {
        code: key,
        title: "損切り価格の方向不正",
        description: extraDetail
          ? `注文方向に対して損切り（SL）価格の設定方向が矛盾しています（${extraDetail}）。`
          : "注文方向（買い/売り）に対して損切り（SL）価格の設定方向が矛盾しています。",
      };
    case "TP_WRONG_SIDE":
      return {
        code: key,
        title: "利確価格の方向不正",
        description: extraDetail
          ? `注文方向に対して利確（TP）価格の設定方向が矛盾しています（${extraDetail}）。`
          : "注文方向（買い/売り）に対して利確（TP）価格の設定方向が矛盾しています。",
      };
    case "NO_SL_TP":
      return {
        code: key,
        title: "損切り/利確の未設定",
        description: extraDetail
          ? `エントリー価格、ストップロス、テイクプロフィットの設定に不備があります（${extraDetail}）。`
          : "エントリー価格、ストップロス、テイクプロフィットのいずれかが設定されていません。",
      };
    case "SPREAD_TOO_WIDE":
      return {
        code: key,
        title: "スプレッド拡大",
        description: extraDetail
          ? `スプレッドが許容最大値を超えています（${extraDetail}）。`
          : log
          ? `スプレッド（${log.spreadPips} pips）が許容最大値を超えて拡大しているため、コスト増を回避し見送りました。`
          : "スプレッドが許容最大値を超えて拡大しているため、コスト増を回避し見送りました。",
      };
    case "MAX_POSITIONS":
      return {
        code: key,
        title: "最大ポジション数上限",
        description: extraDetail
          ? `ポジション数上限に達しています（${extraDetail}）。`
          : "該当通貨ペアまたは口座全体の同時保有ポジション上限に達しているため、新規発注を見送りました。",
      };
    case "DAILY_LOSS_LIMIT":
      return {
        code: key,
        title: "当日損失限度到達",
        description: extraDetail
          ? `当日の確定損失が許容上限に達しています（${extraDetail}）。`
          : "当日の確定損失が許容上限（日次ドローダウン限度）に達しているため、リスク管理のため新規発注を停止しています。",
      };
    case "NEWS_BLACKOUT":
      return {
        code: key,
        title: "重要指標・ニュース発表前後",
        description: extraDetail
          ? `重要経済指標の発表前後の取引停止時間帯に該当します（${extraDetail}）。`
          : "重要経済指標の発表前後の取引停止時間帯に該当するため、突発的な急変動を避けるため発注を見送りました。",
      };
    case "OBSERVED_MISMATCH":
      return {
        code: key,
        title: "観測足データの不整合",
        description: extraDetail
          ? `LLMが参照した直近足データとシステムデータに不整合が検出されました（${extraDetail}）。`
          : "LLMが参照した直近ローソク足データとシステムのスナップショットデータに不整合が検出されました。",
      };
    case "PLAN_UNSTRUCTURED":
      return {
        code: key,
        title: "プラン条件の構造不正",
        description: extraDetail
          ? `条件付き注文の構造不正です（${extraDetail}）。`
          : "条件付き注文のトリガー条件または無効化条件が正しく設定されていません。",
      };
    case "PLAN_NO_SL_TP":
      return {
        code: key,
        title: "プラン損切り/利確未設定",
        description: extraDetail
          ? `条件付き注文のSL/TPが未設定です（${extraDetail}）。`
          : "条件付き注文のストップロスまたはテイクプロフィットが設定されていません。",
      };
    case "PLAN_EXPIRY":
      return {
        code: key,
        title: "プラン有効期限不正",
        description: extraDetail
          ? `条件付き注文の有効期限が不正です（${extraDetail}）。`
          : "条件付き注文の有効期限が切れているか、許容される最大有効期限を超えています。",
      };
    case "PLAN_SL_WRONG_SIDE":
      return {
        code: key,
        title: "プラン損切り価格の方向不正",
        description: extraDetail
          ? `プランの損切り方向不正です（${extraDetail}）。`
          : "プランの注文方向に対して損切り価格の設定方向が矛盾しています。",
      };
    case "PLAN_TP_WRONG_SIDE":
      return {
        code: key,
        title: "プラン利確価格の方向不正",
        description: extraDetail
          ? `プランの利確方向不正です（${extraDetail}）。`
          : "プランの注文方向に対して利確価格の設定方向が矛盾しています。",
      };
    default:
      return {
        code: key,
        title: key.startsWith("ORDER_FAILED") ? "注文執行エラー" : `事後ガードによる抑止 (${key})`,
        description: extraDetail
          ? `${extraDetail}`
          : "リスク管理ルール（事後ガード）の検証に合格しなかったため、安全のため発注を見送りました。",
      };
  }
}

function parseGuardReasons(guardResult?: string, log?: CoTDetail["log"]): GuardReasonInfo[] {
  const guard = splitGuardResult(guardResult).guard;
  if (!guard) return [];
  // 発注エラーの本文には "|" が入りうるので分けない
  if (guard.startsWith("ORDER_FAILED")) return [getGuardReasonDetails(guard, log)];
  return guard
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
  // 条件付きプランは「成立を受けた再判断（旧データは成立ログ）」と「プランを立てた判断」に分かれるため、同じダイアログ内で行き来する
  // 開いた判断（cotLogId）ごとに持ち、別の判断を開き直したら元に戻す
  const [viewing, setViewing] = React.useState<{ base: string | null; id: string } | null>(null);
  const viewingId = viewing?.base === cotLogId ? viewing.id : null;
  const setViewingId = (id: string | null) => setViewing(id ? { base: cotLogId, id } : null);
  const shownId = viewingId ?? cotLogId;

  React.useEffect(() => {
    if (!shownId) return;
    let cancelled = false;
    tradingApi
      .getCoTDetail(shownId)
      .then((res) => {
        if (cancelled) return;
        if (res.success && res.data) setLoaded({ id: shownId, detail: res.data });
        else setLoaded({ id: shownId, error: res.message ?? "判断ログを取得できませんでした" });
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setLoaded({ id: shownId, error: err instanceof Error ? err.message : "判断ログを取得できませんでした" });
      });
    return () => {
      cancelled = true;
    };
  }, [shownId]);

  const current = loaded?.id === shownId ? loaded : null;
  const detail = current?.detail;
  const error = current?.error;
  const log = detail?.log;
  const digits = log ? priceDigits(log.symbol) : 5;
  const guardReasons = React.useMemo(() => parseGuardReasons(log?.guardResult, log), [log]);
  const { guard: guardPart, manage: manageNotes } = React.useMemo(() => splitGuardResult(log?.guardResult), [log]);
  const orderFailed = !!guardPart?.includes("ORDER_FAILED");
  const entryLabel = log?.entryType ? ENTRY_TYPE_LABELS[log.entryType] ?? log.entryType : null;
  const effectiveRR = React.useMemo(() => getEffectiveRiskRewardRatio(log), [log]);

  return (
    <Dialog open={!!cotLogId} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-2xl max-h-[85vh] overflow-y-auto overflow-x-hidden">
        <DialogHeader>
          <div className="flex items-center justify-between pr-6">
            <DialogTitle className="flex items-center gap-2.5 text-lg flex-wrap">
              <BrainCircuit className="h-5 w-5 text-primary shrink-0" />
              <span>判断の詳細{log ? `: ${log.symbol}` : ""}</span>
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
            判断ID: {shownId}
            {log && ` | ${log.timestamp} (UTC) | スプレッド: ${log.spreadPips} pips`}
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
                <span className="text-xs text-muted-foreground">確信度</span>
                <div className="text-lg font-bold font-mono text-primary">
                  {(log.confidence * 100).toFixed(0)}%
                </div>
              </div>

              {log.action !== "HOLD" && (
                <>
                  <div className="min-w-0">
                    <span className="text-xs text-muted-foreground">想定リスクリワード比</span>
                    <div className="text-lg font-bold font-mono text-profit">
                      {effectiveRR != null ? `1 : ${effectiveRR.toFixed(2)}` : "-"}
                    </div>
                  </div>
                  <div className="min-w-0">
                    <span className="text-xs text-muted-foreground">新規注文</span>
                    <div className="text-sm font-semibold mt-1">
                      {log.executed ? (
                        <span className="text-profit flex items-center gap-1">
                          <ShieldCheck className="h-4 w-4 shrink-0" />
                          <span>{entryLabel ? `${entryLabel}を発注` : "発注済み"}</span>
                        </span>
                      ) : orderFailed ? (
                        <span className="text-loss flex items-center gap-1">
                          <ShieldAlert className="h-4 w-4 shrink-0" />
                          <span>未発注（発注エラー）</span>
                        </span>
                      ) : guardPart ? (
                        <span className="text-warning flex items-center gap-1">
                          <ShieldAlert className="h-4 w-4 shrink-0" />
                          <span>未発注（ガード抑止{getGuardShortLabel(log.guardResult) ? `: ${getGuardShortLabel(log.guardResult)}` : ""}）</span>
                        </span>
                      ) : (
                        <span className="text-warning flex items-center gap-1">
                          <ShieldAlert className="h-4 w-4 shrink-0" />
                          <span>未発注（未執行）</span>
                        </span>
                      )}
                    </div>
                  </div>
                </>
              )}
            </div>

            {/* 未発注理由カード (未発注の場合) */}
            {log.action !== "HOLD" && !log.executed && (
              <div
                className={cn(
                  "space-y-2.5 p-3.5 rounded-lg border",
                  orderFailed
                    ? "border-loss/30 bg-loss/5"
                    : "border-warning/30 bg-warning/5"
                )}
              >
                <div className="flex items-start justify-between gap-2 flex-wrap">
                  <div
                    className={cn(
                      "flex min-w-0 items-center gap-2 text-xs font-semibold",
                      orderFailed
                        ? "text-loss"
                        : "text-warning"
                    )}
                  >
                    <ShieldAlert className="h-4 w-4 shrink-0" />
                    <span>
                      {orderFailed
                        ? "未発注理由（注文執行エラー）"
                        : guardReasons.length > 0
                        ? "未発注理由（事後ガードによる抑止）"
                        : "未発注理由（注文未執行）"}
                    </span>
                  </div>
                  {guardPart && (
                    <span className="min-w-0 w-full sm:w-auto max-w-full text-[11px] font-mono text-muted-foreground bg-muted/60 px-1.5 py-0.5 rounded border whitespace-pre-wrap break-all">
                      {guardPart}
                    </span>
                  )}
                </div>

                {guardReasons.length > 0 ? (
                  <div className="space-y-2 pt-0.5">
                    {guardReasons.map((reason) => (
                      <div
                        key={reason.code}
                        className={cn(
                          "p-2.5 rounded-md bg-card/80 border space-y-1 text-xs shadow-xs",
                          reason.code === "ORDER_FAILED"
                            ? "border-loss/30"
                            : "border-warning/20"
                        )}
                      >
                        <div className="flex items-center gap-2 flex-wrap">
                          <span
                            className={cn(
                              "font-semibold",
                              reason.code === "ORDER_FAILED" ? "text-loss" : "text-foreground"
                            )}
                          >
                            {reason.title}
                          </span>
                          <Badge
                            variant="outline"
                            className={cn(
                              "text-[10px] font-mono py-0 px-1.5 h-4",
                              reason.code === "ORDER_FAILED"
                                ? "text-loss border-loss/30"
                                : "text-muted-foreground border-warning/30"
                            )}
                          >
                            {reason.code}
                          </Badge>
                        </div>
                        <p className="text-muted-foreground leading-relaxed whitespace-pre-wrap break-words">
                          {reason.description}
                        </p>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-xs text-muted-foreground leading-relaxed">
                    事後ガードは通過しましたが、注文処理が完了しませんでした（ブローカー未接続または発注処理エラーの可能性があります）。
                  </p>
                )}
              </div>
            )}

            {/* 旧データ: 条件付きプランは「プランを立てた判断」と「成立を受けた再判断」に分かれる。
                cot-plan-* は再判断を導入する前の、LLM を通さずに発注した成立ログ */}
            {(detail.origin || viewingId) && (
              <div className="flex flex-wrap items-center gap-2 p-3 rounded-lg border bg-muted/30">
                <GitBranch className="h-4 w-4 text-muted-foreground shrink-0" />
                <span className="text-xs text-muted-foreground">
                  {!detail.origin
                    ? "条件付きプランを立てた判断です"
                    : detail.log.id.startsWith("cot-plan-")
                      ? "条件付きプランの成立記録です。エントリーの根拠は元の判断にあります"
                      : "条件付きプランの成立を受けて、入るかを判断し直した記録です"}
                </span>
                {detail.origin && (
                  <Button variant="outline" size="sm" onClick={() => setViewingId(detail.origin?.id ?? null)}>
                    プランを立てた判断を見る
                  </Button>
                )}
                {viewingId && (
                  <Button variant="ghost" size="sm" onClick={() => setViewingId(null)}>
                    戻る
                  </Button>
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
                              ? "text-profit"
                              : "text-loss"
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

            {/* 既存の予約注文・建玉への操作 */}
            {manageNotes.length > 0 && (
              <div className="space-y-2 p-3 rounded-lg border bg-card">
                <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                  <ListChecks className="h-4 w-4" />
                  <span>既存の注文・建玉への操作</span>
                </div>
                <ul className="space-y-1.5 text-sm">
                  {manageNotes.map((n, i) => (
                    <li key={i} className="flex flex-wrap items-center justify-between gap-2">
                      <span className="font-medium">{manageNoteLabel(n)}</span>
                      <Badge
                        variant="outline"
                        className={cn("text-[11px] font-mono max-w-full whitespace-normal break-all", n.failed ? "border-loss/40 text-loss" : "text-muted-foreground")}
                      >
                        {n.result}
                      </Badge>
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {/* 注文の価格 (BUY / SELLの場合) */}
            {log.action !== "HOLD" && log.entryPrice && (
              <div className="grid grid-cols-3 gap-2 text-center p-3 rounded-lg border bg-card">
                <div>
                  <span className="text-xs text-muted-foreground">{entryLabel ? `${entryLabel}の価格` : "エントリー価格"}</span>
                  <p className="font-mono text-base font-bold">{log.entryPrice.toFixed(digits)}</p>
                </div>
                <div>
                  <span className="text-xs text-muted-foreground">ストップロス (SL)</span>
                  <p className="font-mono text-base font-bold text-loss">
                    {log.stopLoss?.toFixed(digits)}
                  </p>
                </div>
                <div>
                  <span className="text-xs text-muted-foreground">テイクプロフィット (TP)</span>
                  <p className="font-mono text-base font-bold text-profit">
                    {log.takeProfit?.toFixed(digits)}
                  </p>
                </div>
              </div>
            )}

            {/* ファンダメンタルズの読み（旧データには無い） */}
            {log.fundamentalView && (
              <div className="space-y-1.5 p-3 rounded-lg border bg-card">
                <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                  <Landmark className="h-4 w-4 text-primary" />
                  <span>0. ファンダメンタルズ（事実からの読み）</span>
                </div>
                <p className="text-sm leading-relaxed">{log.fundamentalView}</p>
              </div>
            )}

            {/* 流れの読み（日中短期）／ ファンダ判断の時期は価格の位置 */}
            <div className="space-y-1.5 p-3 rounded-lg border bg-card">
              <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                <TrendingUp className="h-4 w-4 text-primary" />
                <span>{log.fundamentalView ? "1. 価格の位置とエントリー・利確の根拠" : "1. 流れの読み（上位足と直近の値動き）"}</span>
              </div>
              <p className="text-sm leading-relaxed">{log.macroContext}</p>
            </div>

            {/* 注文の価格の根拠（旧: プライスアクション判断の注文の攻防） */}
            {log.orderFlow && (
              <div className="space-y-1.5 p-3 rounded-lg border bg-card">
                <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                  <Sparkles className="h-4 w-4 text-warning" />
                  <span>2. 注文の価格・利確の根拠</span>
                </div>
                <p className="text-sm leading-relaxed font-medium">{log.orderFlow}</p>
              </div>
            )}

            {/* シナリオ無効化価格 (客観的SL理由) */}
            <div className="space-y-1.5 p-3 rounded-lg border border-loss/20 bg-loss/30">
              <div className="flex items-center gap-2 text-xs font-semibold text-loss">
                <ShieldCheck className="h-4 w-4" />
                <span>3. 読みが崩れる価格（損切りの根拠）</span>
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
                <Target className="h-4 w-4 text-primary" />
                <span>5. 判断理由のまとめ</span>
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
