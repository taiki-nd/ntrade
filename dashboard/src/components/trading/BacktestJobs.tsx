"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { ExternalLink, ListTodo, Loader2, Square } from "lucide-react";
import { TIMEFRAME_LABELS } from "@/lib/strategy";
import type { BacktestJob, BacktestJobRequest, BacktestJobStatus, StrategyTimeframe } from "@/types/trading";

const STATUS: Record<BacktestJobStatus, { label: string; variant: "default" | "secondary" | "outline" | "destructive" }> = {
  queued: { label: "待機中", variant: "outline" },
  running: { label: "実行中", variant: "default" },
  succeeded: { label: "完了", variant: "secondary" },
  failed: { label: "失敗", variant: "destructive" },
  cancelled: { label: "中止", variant: "outline" },
};

interface BacktestJobsProps {
  jobs: BacktestJob[];
  onCancel: (id: number) => void;
  onOpenRun: (runId: number) => void;
}

/** 過去データの取得・グリッドサーチ・ウォークフォワードのジョブ（エンジンの中で 1 件ずつ走る） */
export function BacktestJobs({ jobs, onCancel, onOpenRun }: BacktestJobsProps) {
  if (jobs.length === 0) return null;
  return (
    <Card className="shadow-xs">
      <CardHeader className="pb-3">
        <CardTitle className="text-base font-semibold flex items-center gap-2">
          <ListTodo className="h-5 w-5 text-primary" />
          <span>ジョブ</span>
        </CardTitle>
        <CardDescription className="text-xs">
          エンジンの中で 1 件ずつ順番に走ります。記録はエンジンを再起動すると消えます（結果と取得した足は残ります）
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        {jobs.map((j) => {
          const active = j.status === "queued" || j.status === "running";
          const pct = j.total > 0 ? (j.done / j.total) * 100 : 0;
          return (
            <div key={j.id} className="rounded-lg border p-3 space-y-2">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0 space-y-0.5">
                  <div className="flex items-center gap-2">
                    <Badge variant={STATUS[j.status].variant} className="text-[10px] gap-1">
                      {j.status === "running" && <Loader2 className="h-3 w-3 animate-spin" />}
                      {STATUS[j.status].label}
                    </Badge>
                    <span className="text-xs font-medium truncate" title={j.title}>
                      {j.title}
                    </span>
                  </div>
                  <p className="text-[11px] text-muted-foreground font-mono">
                    #{j.id} 受付 {j.createdAt}
                    {j.finishedAt && ` / 終了 ${j.finishedAt}`}
                  </p>
                </div>
                <div className="flex items-center gap-1.5 shrink-0">
                  {j.runId !== null && (
                    <Button variant="outline" size="sm" className="h-7 gap-1 text-xs" onClick={() => onOpenRun(j.runId as number)}>
                      <ExternalLink className="h-3.5 w-3.5" />
                      結果を開く
                    </Button>
                  )}
                  {active && (
                    <Button variant="ghost" size="sm" className="h-7 gap-1 text-xs text-muted-foreground" onClick={() => onCancel(j.id)}>
                      <Square className="h-3.5 w-3.5" />
                      中止
                    </Button>
                  )}
                </div>
              </div>
              {(j.status === "running" || (j.total > 0 && j.status !== "succeeded")) && (
                <div className="flex items-center gap-2">
                  <Progress value={pct} className="h-1.5" />
                  <span className="text-[11px] text-muted-foreground tabular-nums whitespace-nowrap">
                    {j.done.toLocaleString()} / {j.total > 0 ? j.total.toLocaleString() : "…"}
                  </span>
                </div>
              )}
              {j.message && <p className="text-[11px] text-muted-foreground font-mono">{j.message}</p>}
              {j.error && <p className="text-[11px] text-destructive break-all">{j.error}</p>}
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}

const FETCH_PERIODS: StrategyTimeframe[] = ["M1", "M5", "M15", "H1", "H4", "D1"];

interface BacktestFetchFormProps {
  /** 取引設定の対象銘柄 */
  pairs: string[];
  submitting: boolean;
  onSubmit: (req: BacktestJobRequest) => void;
}

/** 過去データの取得（DB に無い期間だけを取る） */
export function BacktestFetchForm({ pairs, submitting, onSubmit }: BacktestFetchFormProps) {
  const [selected, setSelected] = React.useState<string[]>(() => pairs);
  const [extra, setExtra] = React.useState("");
  const [from, setFrom] = React.useState("2023-01-01");
  const [periods, setPeriods] = React.useState<StrategyTimeframe[]>(FETCH_PERIODS);

  const targets = Array.from(
    new Set([...selected, ...extra.split(",").map((p) => p.trim()).filter(Boolean)])
  );
  const valid = targets.length > 0 && periods.length > 0 && !!from;
  const toggle = <T,>(list: T[], v: T, on: boolean) => (on ? [...list, v] : list.filter((x) => x !== v));

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label className="text-xs text-muted-foreground">銘柄</Label>
        <div className="flex flex-wrap gap-3">
          {pairs.map((p) => (
            <label key={p} className="flex items-center gap-1.5 text-xs font-mono cursor-pointer">
              <Checkbox checked={selected.includes(p)} onCheckedChange={(on) => setSelected(toggle(selected, p, !!on))} />
              {p}
            </label>
          ))}
        </div>
        <Input
          value={extra}
          placeholder="ほかの銘柄（カンマ区切り。例: GBPJPY_z）"
          onChange={(e) => setExtra(e.target.value)}
          className="h-8 text-xs font-mono"
        />
      </div>
      <div className="space-y-2">
        <Label className="text-xs text-muted-foreground">時間足</Label>
        <div className="flex flex-wrap gap-3">
          {FETCH_PERIODS.map((p) => (
            <label key={p} className="flex items-center gap-1.5 text-xs font-mono cursor-pointer">
              <Checkbox checked={periods.includes(p)} onCheckedChange={(on) => setPeriods(toggle(periods, p, !!on))} />
              {TIMEFRAME_LABELS[p]}
            </label>
          ))}
        </div>
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="fetch-from" className="text-xs text-muted-foreground">
          開始日 (UTC)。終了は現在まで
        </Label>
        <Input id="fetch-from" type="date" value={from} onChange={(e) => setFrom(e.target.value)} className="h-8 text-xs w-48" />
      </div>
      <p className="text-[11px] text-muted-foreground">
        DB に無い前側・後ろ側の期間だけを cTrader から取り、区切りごとに保存します（途中で中止しても、取れた分は残ります）。1 分足を数年分取ると 1 銘柄 10 分ほどかかります。銘柄の lot サイズなどの仕様も保存します。
      </p>
      <Button
        className="w-full h-9 gap-1.5 text-sm"
        disabled={!valid || submitting}
        onClick={() => onSubmit({ kind: "fetch", pairs: targets, from: `${from}T00:00:00Z`, periods })}
      >
        {submitting && <Loader2 className="h-4 w-4 animate-spin" />}
        取得をジョブとして実行
      </Button>
    </div>
  );
}
