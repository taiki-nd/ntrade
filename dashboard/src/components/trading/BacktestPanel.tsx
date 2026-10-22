"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Database, FlaskConical, Loader2, Play, RefreshCw, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import { tradingApi } from "@/lib/trading-api";
import { BacktestRunForm } from "@/components/trading/BacktestRunForm";
import { BacktestFetchForm, BacktestJobs } from "@/components/trading/BacktestJobs";
import { BacktestRunDetail, KIND_LABELS } from "@/components/trading/BacktestRunDetail";
import type {
  BacktestJob,
  BacktestJobRequest,
  BacktestRunDetail as BacktestRunDetailData,
  BacktestRunRequest,
  BacktestRunRow,
  BarCoverage,
  StrategyInfo,
} from "@/types/trading";

interface BacktestPanelProps {
  /** 取引設定の対象銘柄（実行フォームの候補） */
  pairs: string[];
}

const RUN_LIMIT = 200;
/** ジョブが動いている間の一覧の取り直し間隔 */
const JOB_ACTIVE_INTERVAL = 2000;
const JOB_IDLE_INTERVAL = 15000;

const errorMessage = (err: unknown) =>
  err instanceof Error ? err.message : "エンジンが起動しているか確認してください。";

const amount = new Intl.NumberFormat("ja-JP", { maximumFractionDigits: 0, signDisplay: "exceptZero" });

/** バックテストの実行・結果一覧・結果の詳細 */
export function BacktestPanel({ pairs }: BacktestPanelProps) {
  const [runs, setRuns] = React.useState<BacktestRunRow[]>([]);
  const [strategies, setStrategies] = React.useState<StrategyInfo[]>([]);
  const [coverage, setCoverage] = React.useState<BarCoverage[]>([]);
  const [selected, setSelected] = React.useState<number | null>(null);
  const [detail, setDetail] = React.useState<BacktestRunDetailData | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [detailLoading, setDetailLoading] = React.useState(false);
  const [running, setRunning] = React.useState(false);
  const [formOpen, setFormOpen] = React.useState(false);
  const [fetchOpen, setFetchOpen] = React.useState(false);
  const [submitting, setSubmitting] = React.useState(false);
  const [jobs, setJobs] = React.useState<BacktestJob[]>([]);
  // 削除確認ダイアログの対象。null なら閉じている
  const [pendingDelete, setPendingDelete] = React.useState<BacktestRunRow | null>(null);

  const loadRuns = React.useCallback(async () => {
    setLoading(true);
    try {
      const res = await tradingApi.getBacktestRuns(RUN_LIMIT);
      if (res.success && res.data) {
        const rows = res.data;
        setRuns(rows);
        // 何も選んでいなければ最新の結果を開く
        setSelected((cur) => cur ?? rows[0]?.id ?? null);
      }
    } catch {
      // 疎通エラーはページ上部のアラートで表示する
    } finally {
      setLoading(false);
    }
  }, []);

  // 一覧・戦略の定義・データの範囲は開いたときと「更新」でだけ取る
  const loadAll = React.useCallback(async () => {
    await Promise.all([
      loadRuns(),
      tradingApi
        .getStrategies()
        .then((res) => res.success && res.data && setStrategies(res.data))
        .catch(() => undefined),
      tradingApi
        .getBacktestCoverage()
        .then((res) => res.success && res.data && setCoverage(res.data))
        .catch(() => undefined),
    ]);
  }, [loadRuns]);

  React.useEffect(() => {
    const id = setTimeout(loadAll, 0);
    return () => clearTimeout(id);
  }, [loadAll]);

  React.useEffect(() => {
    if (selected === null) return;
    let cancelled = false;
    const id = setTimeout(() => setDetailLoading(true), 0);
    tradingApi
      .getBacktestRun(selected)
      .then((res) => {
        if (cancelled) return;
        if (res.success && res.data) setDetail(res.data);
        else toast.error("結果を読み込めませんでした", { description: res.message });
      })
      .catch((err) => !cancelled && toast.error("結果を読み込めませんでした", { description: errorMessage(err) }))
      .finally(() => !cancelled && setDetailLoading(false));
    return () => {
      cancelled = true;
      clearTimeout(id);
    };
  }, [selected]);

  // ジョブの一覧。動いている間は短い間隔で取り直し、終わったジョブがあれば結果と足の範囲を読み直す
  const jobsActive = jobs.some((j) => j.status === "queued" || j.status === "running");
  const finishedRef = React.useRef<Set<number> | null>(null);
  const loadJobs = React.useCallback(async () => {
    try {
      const res = await tradingApi.getBacktestJobs();
      if (!res.success || !res.data) return;
      const list = res.data;
      setJobs(list);
      const finished = new Set(list.filter((j) => j.status !== "queued" && j.status !== "running").map((j) => j.id));
      const known = finishedRef.current;
      finishedRef.current = finished;
      if (!known) return;
      const fresh = list.filter((j) => finished.has(j.id) && !known.has(j.id));
      for (const j of fresh) {
        if (j.status === "succeeded") toast.success(`ジョブ #${j.id} が完了しました`, { description: j.title });
        else if (j.status === "failed") toast.error(`ジョブ #${j.id} が失敗しました`, { description: j.error ?? j.title });
      }
      if (fresh.length > 0) {
        await loadRuns();
        const cov = await tradingApi.getBacktestCoverage().catch(() => null);
        if (cov?.success && cov.data) setCoverage(cov.data);
        const opened = fresh.find((j) => j.status === "succeeded" && j.runId !== null);
        if (opened) setSelected(opened.runId);
      }
    } catch {
      // 疎通エラーはページ上部のアラートで表示する
    }
  }, [loadRuns]);

  React.useEffect(() => {
    const first = setTimeout(loadJobs, 0);
    const id = setInterval(loadJobs, jobsActive ? JOB_ACTIVE_INTERVAL : JOB_IDLE_INTERVAL);
    return () => {
      clearTimeout(first);
      clearInterval(id);
    };
  }, [loadJobs, jobsActive]);

  const submitJob = async (req: BacktestJobRequest) => {
    setSubmitting(true);
    try {
      const res = await tradingApi.submitBacktestJob(req);
      if (res.success && res.data) {
        toast.success(res.message || "ジョブを受け付けました", { description: res.data.title });
        setFormOpen(false);
        setFetchOpen(false);
        await loadJobs();
      } else {
        toast.error("ジョブを受け付けられませんでした", { description: res.message });
      }
    } catch (err) {
      toast.error("ジョブを受け付けられませんでした", { description: errorMessage(err) });
    } finally {
      setSubmitting(false);
    }
  };

  const cancelJob = async (id: number) => {
    try {
      const res = await tradingApi.cancelBacktestJob(id);
      if (res.success) toast.success(res.message || "中止を指示しました");
      else toast.error("中止できませんでした", { description: res.message });
    } catch (err) {
      toast.error("中止できませんでした", { description: errorMessage(err) });
    } finally {
      loadJobs();
    }
  };

  const handleRun = async (req: BacktestRunRequest) => {
    setRunning(true);
    try {
      const res = await tradingApi.runBacktest(req);
      if (res.success && res.data) {
        const m = res.data.metrics;
        toast.success(res.message || "バックテストが完了しました", {
          description: `${m.trades} 取引 / 純損益 ${amount.format(m.netProfit)} / PF ${m.profitFactor?.toFixed(2) ?? "—"}`,
        });
        await loadRuns();
        setSelected(res.data.id);
        setFormOpen(false);
      } else {
        toast.error("バックテストを実行できませんでした", { description: res.message });
      }
    } catch (err) {
      toast.error("バックテストを実行できませんでした", { description: errorMessage(err) });
    } finally {
      setRunning(false);
    }
  };

  const confirmDelete = async () => {
    if (!pendingDelete) return;
    const id = pendingDelete.id;
    setPendingDelete(null);
    try {
      const res = await tradingApi.deleteBacktestRun(id);
      if (res.success) {
        toast.info(res.message || "バックテスト結果を削除しました");
        if (selected === id) {
          setSelected(null);
          setDetail(null);
        }
      } else {
        toast.error("削除できませんでした", { description: res.message });
      }
    } catch (err) {
      toast.error("削除できませんでした", { description: errorMessage(err) });
    } finally {
      loadRuns();
    }
  };

  return (
    <div className="space-y-4">
      <BacktestJobs jobs={jobs} onCancel={cancelJob} onOpenRun={setSelected} />
      <div className="space-y-4">
        {/* 実行一覧 */}
        <Card className="shadow-xs min-w-0">
          <CardHeader className="flex flex-col sm:flex-row sm:items-center justify-between pb-3 gap-2">
            <div className="space-y-1">
              <CardTitle className="text-base font-semibold flex items-center gap-2">
                <FlaskConical className="h-5 w-5 text-primary" />
                <span>バックテスト結果</span>
                <Badge variant="secondary" className="font-mono text-xs">
                  {runs.length} 件
                </Badge>
              </CardTitle>
              <CardDescription className="text-xs">
                ヒストリカルの足で戦略を動かした結果。CLI（<code className="font-mono">bin/backtest</code>）で実行した結果もここに並びます
              </CardDescription>
            </div>
            <div className="flex items-center gap-2 shrink-0">
              <Button variant="outline" size="sm" onClick={loadAll} disabled={loading} className="h-8 gap-1.5 text-xs">
                {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
                更新
              </Button>
              <Button variant="outline" size="sm" onClick={() => setFetchOpen(true)} className="h-8 gap-1.5 text-xs">
                <Database className="h-3.5 w-3.5" />
                データを取得
              </Button>
              <Button size="sm" onClick={() => setFormOpen(true)} className="h-8 gap-1.5 text-xs">
                <Play className="h-3.5 w-3.5" />
                新しく実行
              </Button>
            </div>
          </CardHeader>
          <CardContent className="p-0">
            <div className="max-h-[560px] overflow-auto">
              <Table>
                <TableHeader className="bg-muted/40">
                  <TableRow>
                    <TableHead className="text-xs">#</TableHead>
                    <TableHead className="text-xs">種類</TableHead>
                    <TableHead className="text-xs">銘柄 / 戦略</TableHead>
                    <TableHead className="text-xs">期間</TableHead>
                    <TableHead className="text-xs text-right">取引</TableHead>
                    <TableHead className="text-xs text-right">純損益</TableHead>
                    <TableHead className="text-xs text-right">PF</TableHead>
                    <TableHead className="text-xs text-right">最大DD</TableHead>
                    <TableHead className="text-xs">ラベル</TableHead>
                    <TableHead className="w-[44px]" />
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {runs.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={10} className="h-20 text-center text-muted-foreground text-sm">
                        バックテストの結果はまだありません。「新しく実行」から実行してください
                      </TableCell>
                    </TableRow>
                  ) : (
                    runs.map((r) => (
                      <TableRow
                        key={r.id}
                        onClick={() => setSelected(r.id)}
                        data-state={selected === r.id ? "selected" : undefined}
                        className="cursor-pointer"
                      >
                        <TableCell className="text-xs font-mono">{r.id}</TableCell>
                        <TableCell>
                          <Badge variant="outline" className="text-[10px]">
                            {KIND_LABELS[r.kind] ?? r.kind}
                          </Badge>
                        </TableCell>
                        <TableCell className="max-w-[360px]">
                          <div className="text-xs font-semibold">{r.pair}</div>
                          <div className="text-[11px] font-mono text-muted-foreground truncate" title={r.strategy}>
                            {r.strategy}
                          </div>
                        </TableCell>
                        <TableCell className="text-xs font-mono whitespace-nowrap">
                          {r.fromTs.slice(0, 10)}
                          <br />〜 {r.toTs.slice(0, 10)}
                        </TableCell>
                        <TableCell className="text-xs text-right tabular-nums">{r.metrics?.trades ?? "—"}</TableCell>
                        <TableCell
                          className={cn(
                            "text-xs text-right font-mono tabular-nums font-semibold",
                            r.metrics && (r.metrics.netProfit > 0 ? "text-profit" : r.metrics.netProfit < 0 ? "text-loss" : undefined)
                          )}
                        >
                          {r.metrics ? amount.format(r.metrics.netProfit) : "—"}
                        </TableCell>
                        <TableCell className="text-xs text-right font-mono tabular-nums">
                          {r.metrics?.profitFactor != null ? r.metrics.profitFactor.toFixed(2) : "—"}
                        </TableCell>
                        <TableCell className="text-xs text-right font-mono tabular-nums">
                          {r.metrics ? `${r.metrics.maxDrawdownPct.toFixed(1)}%` : "—"}
                        </TableCell>
                        <TableCell className="text-xs max-w-[220px] truncate" title={r.label}>
                          {r.label || <span className="text-muted-foreground">—</span>}
                        </TableCell>
                        <TableCell>
                          <Button
                            variant="ghost"
                            size="icon"
                            aria-label={`#${r.id} を削除`}
                            className="h-7 w-7 text-muted-foreground hover:text-destructive"
                            onClick={(e) => {
                              e.stopPropagation();
                              setPendingDelete(r);
                            }}
                          >
                            <Trash2 className="h-3.5 w-3.5" />
                          </Button>
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </div>
          </CardContent>
        </Card>

      </div>

      {/* 実行フォーム */}
      <Dialog open={formOpen} onOpenChange={(open) => !running && setFormOpen(open)}>
        <DialogContent className="sm:max-w-xl max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              <Play className="h-4 w-4 text-primary" />
              新しく実行
            </DialogTitle>
            <DialogDescription className="text-xs">
              戦略とパラメータを選んで実行します。単発は終わると結果を開き、グリッドサーチ・ウォークフォワードはジョブとして順番に走ります
            </DialogDescription>
          </DialogHeader>
          <BacktestRunForm
            pairs={pairs}
            strategies={strategies}
            coverage={coverage}
            running={running || submitting}
            onRun={handleRun}
            onSubmitJob={submitJob}
          />
        </DialogContent>
      </Dialog>

      {/* 過去データの取得 */}
      <Dialog open={fetchOpen} onOpenChange={setFetchOpen}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              <Database className="h-4 w-4 text-primary" />
              過去データを取得
            </DialogTitle>
            <DialogDescription className="text-xs">cTrader から足を取得して DB に保存します（エンジンの cTrader 接続を使います）</DialogDescription>
          </DialogHeader>
          <BacktestFetchForm key={pairs.join(",")} pairs={pairs} submitting={submitting} onSubmit={submitJob} />
        </DialogContent>
      </Dialog>

      {/* 選んだ実行の結果 */}
      {detailLoading && !detail && (
        <div className="flex items-center justify-center py-10">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      )}
      {detail && <BacktestRunDetail key={detail.run.id} detail={detail} />}

      <Dialog open={!!pendingDelete} onOpenChange={(open) => !open && setPendingDelete(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              <Trash2 className="h-4 w-4 text-destructive" />
              バックテスト結果を削除しますか？
            </DialogTitle>
            <DialogDescription className="text-xs">
              {pendingDelete && `#${pendingDelete.id} ${pendingDelete.pair} ${pendingDelete.strategy} の結果（取引・資産曲線を含む）を削除します。元に戻せません。`}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" size="sm" onClick={() => setPendingDelete(null)}>
              キャンセル
            </Button>
            <Button variant="destructive" size="sm" className="gap-1" onClick={confirmDelete}>
              <Trash2 className="h-3.5 w-3.5" />
              削除する
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
