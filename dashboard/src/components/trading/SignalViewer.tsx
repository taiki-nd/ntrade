"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ChevronDown, ChevronRight, Clock, Loader2, Radio, RefreshCw, Search, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { tradingApi } from "@/lib/trading-api";
import { PaginationBar } from "@/components/trading/PaginationBar";
import type { Page, SignalAction, SignalLog, SignalStatus } from "@/types/trading";

const PAGE_SIZE = 30;
const POLL_INTERVAL = 30000;

const STATUS_LABELS: Record<SignalStatus, string> = {
  EXECUTED: "執行",
  REJECTED: "ガードで却下",
  FAILED: "発注エラー",
  DRY_RUN: "ドライラン",
};

const ACTION_LABELS: Record<SignalAction, string> = {
  OPEN: "新規",
  CLOSE: "決済",
  CANCEL: "取消",
  MODIFY: "変更",
};

const STATUS_FILTER_LABELS: Record<string, string> = { "": "すべて", ...STATUS_LABELS };

/** シグナルの処理結果 */
export function SignalStatusBadge({ status, className }: { status: SignalStatus; className?: string }) {
  const style: Record<SignalStatus, string> = {
    EXECUTED: "border-profit/40 bg-profit/10 text-profit",
    REJECTED: "border-warning/40 bg-warning/10 text-warning",
    FAILED: "border-loss/40 bg-loss/10 text-loss",
    DRY_RUN: "text-muted-foreground",
  };
  return (
    <Badge variant="outline" className={cn("text-[11px]", style[status], className)} title={status}>
      {STATUS_LABELS[status] ?? status}
    </Badge>
  );
}

/** 操作の種類と向き（OPEN BUY など） */
function ActionBadge({ action, side }: { action: SignalAction; side: string | null }) {
  const tone =
    action === "OPEN" && side === "BUY"
      ? "border-profit/40 text-profit"
      : action === "OPEN" && side === "SELL"
        ? "border-loss/40 text-loss"
        : "text-foreground";
  return (
    <Badge variant="outline" className={cn("font-semibold text-[11px]", tone)}>
      {ACTION_LABELS[action] ?? action}
      {side && <span className="font-mono">{side}</span>}
    </Badge>
  );
}

/** ガード結果（"PASS" か、通らなかったチェック名の "|" 区切り） */
function GuardResult({ value }: { value: string | null }) {
  if (!value) return <span className="text-xs text-muted-foreground">—（ガード対象外）</span>;
  if (value === "PASS") {
    return (
      <Badge variant="outline" className="border-profit/40 text-profit text-[11px]">
        PASS
      </Badge>
    );
  }
  return (
    <div className="flex flex-wrap gap-1">
      {value.split("|").map((name) => (
        <Badge key={name} variant="outline" className="border-warning/40 text-warning font-mono text-[11px]">
          {name}
        </Badge>
      ))}
    </div>
  );
}

/** シグナル1件の中身（一覧の展開部分とダイアログで共通） */
export function SignalDetailBody({ signal }: { signal: SignalLog }) {
  return (
    <div className="space-y-3 text-xs">
      <div className="grid grid-cols-[6rem_1fr] gap-x-3 gap-y-1.5">
        <span className="text-muted-foreground">戦略</span>
        <span className="font-mono break-all">{signal.strategy}</span>
        <span className="text-muted-foreground">理由</span>
        <span className="whitespace-pre-wrap">{signal.reason || "—"}</span>
        <span className="text-muted-foreground">ガード</span>
        <GuardResult value={signal.guardResult} />
        <span className="text-muted-foreground">メッセージ</span>
        <span className="whitespace-pre-wrap break-all">{signal.message || "—"}</span>
        <span className="text-muted-foreground">注文 ID</span>
        <span className="font-mono">{signal.orderId ?? "—"}</span>
        <span className="text-muted-foreground">シグナル ID</span>
        <span className="font-mono break-all">{signal.id}</span>
      </div>
      <div className="space-y-1">
        <p className="text-muted-foreground">詳細（操作の中身と、その時点の価格・スプレッド・ATR・ロット）</p>
        <pre className="max-h-80 overflow-auto rounded-md border bg-muted/40 p-3 font-mono text-[11px] leading-relaxed">
          {JSON.stringify(signal.detail, null, 2)}
        </pre>
      </div>
    </div>
  );
}

interface SignalDetailDialogProps {
  /** null なら閉じている */
  signalId: string | null;
  onClose: () => void;
}

/** 建玉・予約注文・取引履歴から、そのもとになったシグナルを開く */
export function SignalDetailDialog({ signalId, onClose }: SignalDetailDialogProps) {
  const [signal, setSignal] = React.useState<SignalLog | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!signalId) return;
    let cancelled = false;
    tradingApi
      .getSignal(signalId)
      .then((res) => {
        if (cancelled) return;
        if (res.success && res.data) {
          setSignal(res.data);
          setError(null);
        } else {
          setError(res.message || "シグナルが見つかりません");
        }
      })
      .catch((err) => !cancelled && setError(err instanceof Error ? err.message : "取得に失敗しました"));
    return () => {
      cancelled = true;
      setSignal(null);
      setError(null);
    };
  }, [signalId]);

  return (
    <Dialog open={!!signalId} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex flex-wrap items-center gap-2 text-base">
            <Radio className="h-4 w-4 text-primary" />
            シグナル
            {signal && (
              <>
                <span className="font-semibold">{signal.symbol}</span>
                <ActionBadge action={signal.action} side={signal.side} />
                <SignalStatusBadge status={signal.status} />
              </>
            )}
          </DialogTitle>
          <DialogDescription className="text-xs font-mono">
            {signal ? `${signal.timestamp} (UTC) · ${signal.deployment}` : error ?? "読み込み中..."}
          </DialogDescription>
        </DialogHeader>
        {signal ? (
          <SignalDetailBody signal={signal} />
        ) : (
          !error && <Loader2 className="h-5 w-5 animate-spin text-muted-foreground mx-auto" />
        )}
      </DialogContent>
    </Dialog>
  );
}

interface SignalViewerProps {
  /** 銘柄プルダウンの選択肢 */
  pairs: string[];
  /** デプロイ ID の選択肢 */
  deployments: string[];
}

/** 戦略が出した操作（新規・決済・取消・変更）と、その処理結果の記録 */
export function SignalViewer({ pairs, deployments }: SignalViewerProps) {
  const [page, setPage] = React.useState(1);
  const [data, setData] = React.useState<Page<SignalLog>>({ items: [], total: 0 });
  const [symbol, setSymbol] = React.useState("");
  const [deployment, setDeployment] = React.useState("");
  const [status, setStatus] = React.useState<"" | SignalStatus>("");
  // 入力中は打鍵ごとに取得しないよう遅延させる
  const [query, setQuery] = React.useState("");
  const [queryDebounced, setQueryDebounced] = React.useState("");
  const [expanded, setExpanded] = React.useState<string[]>([]);
  const [loading, setLoading] = React.useState(false);

  React.useEffect(() => {
    const t = setTimeout(() => setQueryDebounced(query.trim()), 300);
    return () => clearTimeout(t);
  }, [query]);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const res = await tradingApi.getSignals({
        limit: PAGE_SIZE,
        offset: (page - 1) * PAGE_SIZE,
        symbol: symbol || undefined,
        deployment: deployment || undefined,
        status: status || undefined,
        q: queryDebounced || undefined,
      });
      if (res.success && res.data) setData(res.data);
    } catch {
      // 疎通エラーはページ上部のアラートで表示する
    } finally {
      setLoading(false);
    }
  }, [page, symbol, deployment, status, queryDebounced]);

  // 条件が変われば即時取得し、以降は一定間隔で取り直す
  React.useEffect(() => {
    const initial = setTimeout(load, 0);
    const interval = setInterval(load, POLL_INTERVAL);
    return () => {
      clearTimeout(initial);
      clearInterval(interval);
    };
  }, [load]);

  const withReset = <T,>(setter: (v: T) => void) => (v: T) => {
    setter(v);
    setPage(1);
  };

  const toggle = (id: string) =>
    setExpanded((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]));

  const hasCondition = !!(symbol || deployment || status || queryDebounced);
  // 設定から外したデプロイの記録も絞り込めるよう、表示中の行の ID も候補に入れる
  const deploymentOptions = Array.from(new Set([...deployments, ...data.items.map((s) => s.deployment)]));

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-col lg:flex-row lg:items-center justify-between gap-3 pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <Radio className="h-5 w-5 text-primary" />
            <span>シグナル</span>
          </CardTitle>
          <CardDescription className="text-xs">
            戦略が足の確定ごとに出した操作と、ガード・発注の結果。ドライランのデプロイは発注せずここに記録だけします
          </CardDescription>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Select
            value={symbol}
            items={{ "": "全銘柄", ...Object.fromEntries(pairs.map((p) => [p, p])) }}
            onValueChange={(v) => withReset(setSymbol)(String(v ?? ""))}
          >
            <SelectTrigger size="sm" className="h-8 w-32 text-xs" aria-label="銘柄で絞り込み">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="">全銘柄</SelectItem>
              {pairs.map((p) => (
                <SelectItem key={p} value={p}>
                  {p}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            value={deployment}
            items={{ "": "全デプロイ", ...Object.fromEntries(deploymentOptions.map((d) => [d, d])) }}
            onValueChange={(v) => withReset(setDeployment)(String(v ?? ""))}
          >
            <SelectTrigger size="sm" className="h-8 w-40 text-xs" aria-label="デプロイで絞り込み">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="">全デプロイ</SelectItem>
              {deploymentOptions.map((d) => (
                <SelectItem key={d} value={d}>
                  {d}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            value={status}
            items={STATUS_FILTER_LABELS}
            onValueChange={(v) => withReset(setStatus)((v ?? "") as "" | SignalStatus)}
          >
            <SelectTrigger size="sm" className="h-8 w-32 text-xs" aria-label="結果で絞り込み">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {Object.entries(STATUS_FILTER_LABELS).map(([v, l]) => (
                <SelectItem key={v} value={v}>
                  {l}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <div className="relative w-full sm:w-56">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground pointer-events-none" />
            <Input
              value={query}
              onChange={(e) => withReset(setQuery)(e.target.value)}
              placeholder="理由・ガード結果・メッセージを検索"
              aria-label="シグナルを検索"
              className="h-8 pl-8 pr-8 text-xs"
            />
            {query && (
              <Button
                variant="ghost"
                size="icon"
                aria-label="検索語をクリア"
                className="absolute right-0.5 top-1/2 -translate-y-1/2 h-7 w-7"
                onClick={() => withReset(setQuery)("")}
              >
                <X className="h-3.5 w-3.5" />
              </Button>
            )}
          </div>
          <Button variant="outline" size="sm" onClick={load} disabled={loading} className="h-8 gap-1.5 text-xs">
            {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
            更新
          </Button>
        </div>
      </CardHeader>

      <CardContent className="space-y-2">
        {data.items.length === 0 && (
          <p className="text-sm text-muted-foreground text-center py-6">
            {hasCondition ? "条件に一致するシグナルはありません" : "シグナルはまだありません。デプロイを有効にすると、足の確定ごとに記録されます"}
          </p>
        )}
        {data.items.map((s) => {
          const open = expanded.includes(s.id);
          return (
            <div key={s.id} className="rounded-lg border bg-card/60">
              <div
                onClick={() => toggle(s.id)}
                className="flex items-start gap-3 p-3 hover:bg-accent/40 cursor-pointer transition-colors rounded-lg"
              >
                <Button
                  variant="ghost"
                  size="icon-xs"
                  className="mt-0.5 shrink-0"
                  aria-label={open ? "詳細を閉じる" : "詳細を開く"}
                  aria-expanded={open}
                >
                  {open ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
                </Button>
                <div className="min-w-0 flex-1 space-y-1">
                  <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                    <span className="font-semibold text-sm">{s.symbol}</span>
                    <ActionBadge action={s.action} side={s.side} />
                    <SignalStatusBadge status={s.status} />
                    {s.guardResult && s.guardResult !== "PASS" && (
                      <span className="text-[11px] font-mono text-warning truncate" title={s.guardResult}>
                        {s.guardResult.split("|").join(", ")}
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground line-clamp-2" title={s.reason}>
                    {s.reason || "—"}
                  </p>
                  <div className="flex flex-wrap items-center gap-x-3 gap-y-0.5 text-[11px] text-muted-foreground font-mono">
                    <span className="flex items-center gap-1">
                      <Clock className="h-3 w-3 shrink-0" />
                      {s.timestamp}
                    </span>
                    <span title={s.strategy}>{s.deployment}</span>
                  </div>
                </div>
              </div>
              {open && (
                <div className="border-t px-4 py-3">
                  <SignalDetailBody signal={s} />
                </div>
              )}
            </div>
          );
        })}

        <PaginationBar page={page} pageSize={PAGE_SIZE} total={data.total} onPageChange={setPage} />
      </CardContent>
    </Card>
  );
}
