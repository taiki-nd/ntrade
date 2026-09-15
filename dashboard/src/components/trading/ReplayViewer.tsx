"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { FlaskConical, RefreshCw, Loader2 } from "lucide-react";
import {
  tradingApi,
  type ReplayRun,
  type ReplayRunDetail,
  type ReplayBucket,
  type ReplayCoverage,
} from "@/lib/trading-api";

function pct(v: number | null | undefined): string {
  return v === null || v === undefined ? "-" : `${(v * 100).toFixed(0)}%`;
}
function num(v: number | null | undefined, d = 1): string {
  return v === null || v === undefined ? "-" : v.toFixed(d);
}

function BucketRows({ rows }: { rows: [string, ReplayBucket][] }) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead className="text-xs">帯</TableHead>
          <TableHead className="text-xs text-right">n</TableHead>
          <TableHead className="text-xs text-right">trades</TableHead>
          <TableHead className="text-xs text-right">TP</TableHead>
          <TableHead className="text-xs text-right">SL</TableHead>
          <TableHead className="text-xs text-right">同一足</TableHead>
          <TableHead className="text-xs text-right">TO</TableHead>
          <TableHead className="text-xs text-right">勝率</TableHead>
          <TableHead className="text-xs text-right">平均pips</TableHead>
          <TableHead className="text-xs text-right">合計pips</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map(([k, b]) => (
          <TableRow key={k}>
            <TableCell className="text-xs font-mono">{k}</TableCell>
            <TableCell className="text-xs text-right">{b.n}</TableCell>
            <TableCell className="text-xs text-right">{b.trades}</TableCell>
            <TableCell className="text-xs text-right">{b.tp_hit}</TableCell>
            <TableCell className="text-xs text-right">{b.sl_hit}</TableCell>
            <TableCell className="text-xs text-right">{b.same_bar}</TableCell>
            <TableCell className="text-xs text-right">{b.timeout}</TableCell>
            <TableCell className="text-xs text-right">{pct(b.win_rate)}</TableCell>
            <TableCell className="text-xs text-right">{num(b.avg_pnl_pips)}</TableCell>
            <TableCell className="text-xs text-right">{num(b.total_pnl_pips)}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

export function ReplayViewer() {
  const [runs, setRuns] = React.useState<ReplayRun[]>([]);
  const [coverage, setCoverage] = React.useState<ReplayCoverage[]>([]);
  const [selected, setSelected] = React.useState<number | null>(null);
  const [detail, setDetail] = React.useState<ReplayRunDetail | null>(null);
  const [loading, setLoading] = React.useState(false);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const [r, c] = await Promise.all([
        tradingApi.getReplayRuns().catch(() => null),
        tradingApi.getReplayCoverage().catch(() => null),
      ]);
      if (r?.success && r.data) setRuns(r.data);
      if (c?.success && c.data) setCoverage(c.data);
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    const id = setTimeout(load, 0);
    return () => clearTimeout(id);
  }, [load]);

  React.useEffect(() => {
    if (selected === null) return;
    let cancelled = false;
    tradingApi
      .getReplayRun(selected)
      .then((res) => {
        if (!cancelled && res.success && res.data) setDetail(res.data);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [selected]);

  const report = detail?.report;

  return (
    <div className="space-y-4">
      <Card className="shadow-xs">
        <CardHeader className="flex flex-col sm:flex-row sm:items-center justify-between pb-3 gap-2">
          <div className="space-y-1">
            <CardTitle className="text-base font-semibold flex items-center gap-2">
              <FlaskConical className="h-5 w-5 text-primary" />
              <span>リプレイ結果</span>
            </CardTitle>
            <CardDescription className="text-xs">
              過去スナップショットに対する LLM 判断の採点。確信度帯ごとの勝率でガード閾値を較正する。
            </CardDescription>
          </div>
          <Button variant="outline" size="sm" onClick={load} disabled={loading} className="h-8 gap-1.5 text-xs">
            {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
            更新
          </Button>
        </CardHeader>
        <CardContent className="space-y-3">
          {coverage.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {coverage.map((c) => (
                <Badge key={`${c.pair}-${c.period}`} variant="outline" className="text-[11px] font-mono">
                  {c.pair} {c.period}: {c.count} 本 ({c.first?.slice(0, 10)} 〜 {c.last?.slice(0, 10)})
                </Badge>
              ))}
            </div>
          )}
          {runs.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              リプレイ実行がまだありません。<code className="font-mono text-xs">cargo run --bin replay -- run ...</code> で実行してください。
            </p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="text-xs">#</TableHead>
                  <TableHead className="text-xs">作成</TableHead>
                  <TableHead className="text-xs">ペア</TableHead>
                  <TableHead className="text-xs">期間</TableHead>
                  <TableHead className="text-xs">サンプリング</TableHead>
                  <TableHead className="text-xs text-right">判断数</TableHead>
                  <TableHead className="text-xs">ラベル</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {runs.map((r) => (
                  <TableRow
                    key={r.id}
                    onClick={() => setSelected(r.id)}
                    data-state={selected === r.id ? "selected" : undefined}
                    className="cursor-pointer"
                  >
                    <TableCell className="text-xs font-mono">{r.id}</TableCell>
                    <TableCell className="text-xs">{r.created_at}</TableCell>
                    <TableCell className="text-xs">{r.pair}</TableCell>
                    <TableCell className="text-xs">
                      {r.from_ts.slice(0, 10)} 〜 {r.to_ts.slice(0, 10)}
                    </TableCell>
                    <TableCell className="text-xs font-mono">{r.sampling}</TableCell>
                    <TableCell className="text-xs text-right">{r.decisions}</TableCell>
                    <TableCell className="text-xs">{r.label}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {report && detail && (
        <Card className="shadow-xs">
          <CardHeader className="pb-3">
            <CardTitle className="text-base font-semibold">Run #{report.run_id} の集計</CardTitle>
            <CardDescription className="text-xs flex flex-wrap gap-x-4 gap-y-1">
              <span>判断 {report.total}</span>
              <span>ガード通過 {report.guard_pass}</span>
              <span>HOLD率 {pct(report.hold_rate)}</span>
              <span>条件付きプラン率 {pct(report.plan_rate)} (成立 {report.plan_triggered})</span>
              <span>画像未読率 {pct(report.unobserved_rate)}</span>
              <span>反対材料なし率 {pct(report.conflicts_empty_rate)}</span>
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex flex-wrap gap-1.5">
              {Object.entries(report.by_outcome).map(([k, v]) => (
                <Badge key={k} variant="outline" className="text-[11px] font-mono">
                  {k}: {v}
                </Badge>
              ))}
              {Object.entries(report.by_guard)
                .filter(([k]) => k !== "PASS")
                .map(([k, v]) => (
                  <Badge key={k} variant="secondary" className="text-[11px] font-mono">
                    guard {k}: {v}
                  </Badge>
                ))}
            </div>

            <div>
              <p className="text-xs font-medium mb-1">全体（ガード通過 / 弾かれた判断を仮執行した場合）</p>
              <BucketRows rows={[["PASS", report.overall], ["rejected", report.rejected]]} />
            </div>
            <div>
              <p className="text-xs font-medium mb-1">確信度帯別</p>
              <BucketRows rows={Object.entries(report.by_confidence)} />
            </div>
            <div>
              <p className="text-xs font-medium mb-1">セッション別</p>
              <BucketRows rows={Object.entries(report.by_session)} />
            </div>

            <div>
              <p className="text-xs font-medium mb-1">判断一覧</p>
              <div className="max-h-80 overflow-auto rounded-md border">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead className="text-xs">t</TableHead>
                      <TableHead className="text-xs">セッション</TableHead>
                      <TableHead className="text-xs">action</TableHead>
                      <TableHead className="text-xs text-right">conf</TableHead>
                      <TableHead className="text-xs">guard</TableHead>
                      <TableHead className="text-xs">outcome</TableHead>
                      <TableHead className="text-xs text-right">pips</TableHead>
                      <TableHead className="text-xs text-right">bars</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {detail.decisions.map((d) => (
                      <TableRow key={d.t}>
                        <TableCell className="text-xs font-mono">{d.t}</TableCell>
                        <TableCell className="text-xs">{d.session}</TableCell>
                        <TableCell className="text-xs font-medium">{d.action}</TableCell>
                        <TableCell className="text-xs text-right">{d.confidence.toFixed(2)}</TableCell>
                        <TableCell className="text-xs font-mono">{d.guard_result}</TableCell>
                        <TableCell className="text-xs font-mono">{d.outcome}</TableCell>
                        <TableCell className="text-xs text-right">{num(d.pnl_pips)}</TableCell>
                        <TableCell className="text-xs text-right">{d.bars_to_exit ?? "-"}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
