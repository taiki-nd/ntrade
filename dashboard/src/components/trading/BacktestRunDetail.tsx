"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ChartNoAxesCombined } from "lucide-react";
import { cn } from "@/lib/utils";
import { priceDigits } from "@/lib/price";
import { formatParams } from "@/lib/strategy";
import { SideBadge } from "@/components/trading/PositionTable";
import { EquityChart } from "@/components/trading/EquityChart";
import { PaginationBar } from "@/components/trading/PaginationBar";
import type {
  BacktestKind,
  BacktestRunDetail as BacktestRunDetailData,
  BacktestSingleConfig,
  BacktestTrade,
  Metrics,
  SweepRow,
  WalkForwardWindow,
} from "@/types/trading";

const TRADE_PAGE_SIZE = 50;

export const KIND_LABELS: Record<BacktestKind, string> = {
  single: "単発",
  sweep: "グリッドサーチ",
  walk_forward: "ウォークフォワード",
};

const EXIT_REASON_LABELS: Record<string, string> = {
  TAKE_PROFIT: "利確 (TP)",
  STOP_LOSS: "損切り (SL)",
  STRATEGY: "戦略決済",
  END_OF_TEST: "テスト終了で決済",
};

const ENTRY_KIND_LABELS: Record<string, string> = { MARKET: "成行", LIMIT: "指値", STOP: "逆指値" };

type Money = (v: number, signed?: boolean) => string;

/** 口座通貨で金額を表示する（通貨コードが読めなければ数値のみ） */
export function moneyFormatter(currency: string | undefined): Money {
  const make = (signed: boolean) => {
    try {
      return new Intl.NumberFormat("ja-JP", {
        style: currency ? "currency" : "decimal",
        currency: currency || undefined,
        maximumFractionDigits: currency === "JPY" || !currency ? 0 : 2,
        signDisplay: signed ? "exceptZero" : "auto",
      });
    } catch {
      return new Intl.NumberFormat("ja-JP", { maximumFractionDigits: 2, signDisplay: signed ? "exceptZero" : "auto" });
    }
  };
  const plain = make(false);
  const signedFmt = make(true);
  return (v, signed = false) => (signed ? signedFmt : plain).format(v);
}

const num = (v: number | null | undefined, digits = 2) => (v === null || v === undefined || !Number.isFinite(v) ? "—" : v.toFixed(digits));
const pnlClass = (v: number) => (v > 0 ? "text-profit" : v < 0 ? "text-loss" : undefined);
/** RFC3339 を "YYYY-MM-DD HH:MM" に */
const shortTime = (s: string) => s.replace("T", " ").replace(/:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/, "");
const day = (s: string) => s.slice(0, 10);

/** run.config から単発実行の条件を取り出す（sweep / walk_forward も config.config に同じ形を持つ） */
function singleConfig(config: unknown): BacktestSingleConfig {
  return config && typeof config === "object" ? (config as BacktestSingleConfig) : {};
}

function MetricCard({ label, value, sub, valueClassName }: { label: string; value: string; sub?: string; valueClassName?: string }) {
  return (
    <div className="rounded-lg border bg-card px-3 py-2.5">
      <p className="text-[11px] text-muted-foreground">{label}</p>
      <p className={cn("text-lg font-semibold font-mono tabular-nums tracking-tight", valueClassName)}>{value}</p>
      {sub && <p className="text-[11px] font-mono tabular-nums text-muted-foreground">{sub}</p>}
    </div>
  );
}

/** 主要な成績のカードと補足の数字 */
export function MetricsSummary({ metrics: m, money }: { metrics: Metrics; money: Money }) {
  return (
    <div className="space-y-2">
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
        <MetricCard
          label="純損益"
          value={money(m.netProfit, true)}
          sub={`${m.returnPct > 0 ? "+" : ""}${num(m.returnPct)}% · ${m.totalPips > 0 ? "+" : ""}${num(m.totalPips, 1)} pips`}
          valueClassName={pnlClass(m.netProfit)}
        />
        <MetricCard label="勝率" value={`${num(m.winRate, 1)}%`} sub={`${m.wins}勝 ${m.losses}敗`} />
        <MetricCard
          label="プロフィットファクター"
          value={num(m.profitFactor)}
          sub={`ペイオフ ${num(m.payoffRatio)}`}
          valueClassName={m.profitFactor !== null ? (m.profitFactor >= 1 ? "text-profit" : "text-loss") : undefined}
        />
        <MetricCard label="最大ドローダウン" value={`${num(m.maxDrawdownPct)}%`} sub={money(-m.maxDrawdown)} valueClassName="text-loss" />
        <MetricCard label="シャープレシオ" value={num(m.sharpe)} sub={`リカバリー ${num(m.recoveryFactor)}`} />
        <MetricCard
          label="取引数"
          value={String(m.trades)}
          sub={`買 ${m.longTrades} (${num(m.longWinRate, 0)}%) / 売 ${m.shortTrades} (${num(m.shortWinRate, 0)}%)`}
        />
        <MetricCard
          label="期待値 (1取引)"
          value={money(m.expectancy, true)}
          sub={`${m.avgPips > 0 ? "+" : ""}${num(m.avgPips, 1)} pips`}
          valueClassName={pnlClass(m.expectancy)}
        />
        <MetricCard label="手数料" value={money(-Math.abs(m.commission))} sub={`スワップ ${money(m.swap, true)}`} />
      </div>
      <div className="flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-muted-foreground font-mono tabular-nums">
        <span>
          残高 {money(m.initialBalance)} → {money(m.finalBalance)}
        </span>
        <span>
          総利益 {money(m.grossProfit)} / 総損失 {money(-Math.abs(m.grossLoss))}
        </span>
        <span>
          平均勝ち {money(m.avgWin)} / 平均負け {money(-Math.abs(m.avgLoss))}
        </span>
        <span>
          最大連勝 {m.maxConsecutiveWins} / 最大連敗 {m.maxConsecutiveLosses}
        </span>
        <span>平均保有 {num(m.avgHoldMinutes / 60, 1)} 時間</span>
      </div>
    </div>
  );
}

function MonthlyTable({ metrics, money }: { metrics: Metrics; money: Money }) {
  if (metrics.monthly.length === 0) return <p className="text-xs text-muted-foreground">月別の成績はありません</p>;
  return (
    <div className="max-h-80 overflow-auto rounded-md border">
      <Table>
        <TableHeader className="bg-muted/40">
          <TableRow>
            <TableHead className="text-xs">月</TableHead>
            <TableHead className="text-xs text-right">取引</TableHead>
            <TableHead className="text-xs text-right">pips</TableHead>
            <TableHead className="text-xs text-right">純損益</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {metrics.monthly.map((r) => (
            <TableRow key={r.month}>
              <TableCell className="text-xs font-mono">{r.month}</TableCell>
              <TableCell className="text-xs text-right tabular-nums">{r.trades}</TableCell>
              <TableCell className={cn("text-xs text-right font-mono tabular-nums", pnlClass(r.pips))}>{num(r.pips, 1)}</TableCell>
              <TableCell className={cn("text-xs text-right font-mono tabular-nums", pnlClass(r.net))}>{money(r.net, true)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

/** 件数の内訳（決済理由・ガードで弾いた理由） */
function CountList({ counts, labels }: { counts: Record<string, number>; labels?: Record<string, string> }) {
  const rows = Object.entries(counts).sort((a, b) => b[1] - a[1]);
  const total = rows.reduce((s, [, v]) => s + v, 0);
  if (rows.length === 0) return <p className="text-xs text-muted-foreground">なし</p>;
  return (
    <div className="space-y-1">
      {rows.map(([k, v]) => (
        <div key={k} className="flex items-center gap-2 text-xs">
          <span className="w-36 shrink-0 truncate" title={k}>
            {labels?.[k] ?? <span className="font-mono">{k}</span>}
          </span>
          <div className="h-1.5 flex-1 rounded-full bg-muted overflow-hidden">
            <div className="h-full bg-primary/60" style={{ width: `${total > 0 ? (v / total) * 100 : 0}%` }} />
          </div>
          <span className="w-10 text-right font-mono tabular-nums">{v}</span>
        </div>
      ))}
    </div>
  );
}

function TradesTable({ trades, pair, money }: { trades: BacktestTrade[]; pair: string; money: Money }) {
  const [page, setPage] = React.useState(1);
  const d = priceDigits(pair);
  const rows = trades.slice((page - 1) * TRADE_PAGE_SIZE, page * TRADE_PAGE_SIZE);
  return (
    <div className="space-y-2">
      <div className="overflow-x-auto rounded-md border">
        <Table>
          <TableHeader className="bg-muted/40">
            <TableRow>
              <TableHead className="text-xs">エントリー (UTC)</TableHead>
              <TableHead className="text-xs">売買</TableHead>
              <TableHead className="text-xs text-right">数量</TableHead>
              <TableHead className="text-xs text-right">建値 → 決済</TableHead>
              <TableHead className="text-xs text-right">SL / TP</TableHead>
              <TableHead className="text-xs text-right">pips</TableHead>
              <TableHead className="text-xs text-right">純損益</TableHead>
              <TableHead className="text-xs text-right" title="最大逆行 / 最大順行（pips）">MAE / MFE</TableHead>
              <TableHead className="text-xs">決済</TableHead>
              <TableHead className="text-xs">理由</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((t) => (
              <TableRow key={String(t.id)}>
                <TableCell className="text-xs font-mono whitespace-nowrap" title={`決済 ${shortTime(t.exitTime)}`}>
                  {shortTime(t.entryTime)}
                </TableCell>
                <TableCell>
                  <SideBadge side={t.side} suffix={t.entryKind !== "MARKET" ? ENTRY_KIND_LABELS[t.entryKind] : undefined} />
                </TableCell>
                <TableCell className="text-xs text-right font-mono tabular-nums">{t.volumeLots.toFixed(2)}</TableCell>
                <TableCell className="text-xs text-right font-mono tabular-nums whitespace-nowrap">
                  <span className="text-muted-foreground">{t.entryPrice.toFixed(d)} → </span>
                  {t.exitPrice.toFixed(d)}
                </TableCell>
                <TableCell className="text-[11px] text-right font-mono tabular-nums whitespace-nowrap">
                  <span className="text-loss">{t.stopLoss != null ? t.stopLoss.toFixed(d) : "—"}</span>
                  {" / "}
                  <span className="text-profit">{t.takeProfit != null ? t.takeProfit.toFixed(d) : "—"}</span>
                </TableCell>
                <TableCell className={cn("text-xs text-right font-mono tabular-nums", pnlClass(t.pnlPips))}>{num(t.pnlPips, 1)}</TableCell>
                <TableCell
                  className={cn("text-xs text-right font-mono tabular-nums font-semibold", pnlClass(t.net))}
                  title={`損益 ${money(t.gross, true)} / 手数料 ${money(-Math.abs(t.commission))} / スワップ ${money(t.swap, true)}`}
                >
                  {money(t.net, true)}
                </TableCell>
                <TableCell className="text-[11px] text-right font-mono tabular-nums text-muted-foreground">
                  {num(t.maePips, 1)} / {num(t.mfePips, 1)}
                </TableCell>
                <TableCell className="text-xs whitespace-nowrap" title={t.exitNote || undefined}>
                  {EXIT_REASON_LABELS[t.exitReason] ?? t.exitReason}
                </TableCell>
                <TableCell className="text-[11px] text-muted-foreground max-w-[240px] truncate" title={[t.entryReason, t.exitNote].filter(Boolean).join("\n")}>
                  {t.entryReason || "—"}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
      <PaginationBar page={page} pageSize={TRADE_PAGE_SIZE} total={trades.length} onPageChange={setPage} />
    </div>
  );
}

function SweepTable({ rows, money }: { rows: SweepRow[]; money: Money }) {
  return (
    <div className="max-h-[480px] overflow-auto rounded-md border">
      <Table>
        <TableHeader className="bg-muted/40">
          <TableRow>
            <TableHead className="text-xs">#</TableHead>
            <TableHead className="text-xs">パラメータ</TableHead>
            <TableHead className="text-xs text-right">取引</TableHead>
            <TableHead className="text-xs text-right">純損益</TableHead>
            <TableHead className="text-xs text-right">PF</TableHead>
            <TableHead className="text-xs text-right">勝率</TableHead>
            <TableHead className="text-xs text-right">最大DD</TableHead>
            <TableHead className="text-xs text-right">シャープ</TableHead>
            <TableHead className="text-xs text-right">スコア</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.map((r, i) => (
            <TableRow key={i} data-state={i === 0 ? "selected" : undefined}>
              <TableCell className="text-xs font-mono">{i + 1}</TableCell>
              <TableCell className="text-xs font-mono">{formatParams(r.params) || "既定"}</TableCell>
              <TableCell className="text-xs text-right tabular-nums">{r.metrics.trades}</TableCell>
              <TableCell className={cn("text-xs text-right font-mono tabular-nums", pnlClass(r.metrics.netProfit))}>
                {money(r.metrics.netProfit, true)}
              </TableCell>
              <TableCell className="text-xs text-right font-mono tabular-nums">{num(r.metrics.profitFactor)}</TableCell>
              <TableCell className="text-xs text-right font-mono tabular-nums">{num(r.metrics.winRate, 1)}%</TableCell>
              <TableCell className="text-xs text-right font-mono tabular-nums">{num(r.metrics.maxDrawdownPct)}%</TableCell>
              <TableCell className="text-xs text-right font-mono tabular-nums">{num(r.metrics.sharpe)}</TableCell>
              <TableCell className="text-xs text-right font-mono tabular-nums font-semibold">{num(r.score, 3)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

function WindowsTable({ windows, money }: { windows: WalkForwardWindow[]; money: Money }) {
  return (
    <div className="overflow-x-auto rounded-md border">
      <Table>
        <TableHeader className="bg-muted/40">
          <TableRow>
            <TableHead className="text-xs">#</TableHead>
            <TableHead className="text-xs">学習期間</TableHead>
            <TableHead className="text-xs">テスト期間</TableHead>
            <TableHead className="text-xs">選んだパラメータ</TableHead>
            <TableHead className="text-xs text-right">学習 純損益</TableHead>
            <TableHead className="text-xs text-right">テスト 純損益</TableHead>
            <TableHead className="text-xs text-right">テスト 取引 / PF</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {windows.map((w, i) => (
            <TableRow key={i}>
              <TableCell className="text-xs font-mono">{i + 1}</TableCell>
              <TableCell className="text-xs font-mono whitespace-nowrap">
                {day(w.trainFrom)} 〜 {day(w.trainTo)}
              </TableCell>
              <TableCell className="text-xs font-mono whitespace-nowrap">
                {day(w.testFrom)} 〜 {day(w.testTo)}
              </TableCell>
              <TableCell className="text-xs font-mono">{w.params ? formatParams(w.params) || "既定" : "選べず（取引なし）"}</TableCell>
              <TableCell className={cn("text-xs text-right font-mono tabular-nums", w.train && pnlClass(w.train.netProfit))}>
                {w.train ? money(w.train.netProfit, true) : "—"}
              </TableCell>
              <TableCell className={cn("text-xs text-right font-mono tabular-nums font-semibold", w.test && pnlClass(w.test.netProfit))}>
                {w.test ? money(w.test.netProfit, true) : "—"}
              </TableCell>
              <TableCell className="text-xs text-right font-mono tabular-nums">
                {w.test ? `${w.test.trades} / ${num(w.test.profitFactor)}` : "—"}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

interface BacktestRunDetailProps {
  detail: BacktestRunDetailData;
}

/** バックテスト1件の結果 */
export function BacktestRunDetail({ detail }: BacktestRunDetailProps) {
  const { run, trades, equity } = detail;
  const cfg = singleConfig(run.config);
  const money = React.useMemo(() => moneyFormatter(cfg.config?.accountCurrency), [cfg.config?.accountCurrency]);
  const sweepRows = run.kind === "sweep" && Array.isArray(run.summary) ? (run.summary as SweepRow[]) : [];
  const windows = run.kind === "walk_forward" && Array.isArray(run.summary) ? (run.summary as WalkForwardWindow[]) : [];
  const costs = cfg.config?.costs;
  const signals = cfg.signals;

  return (
    <div className="space-y-4">
      <Card className="shadow-xs">
        <CardHeader className="pb-3">
          <CardTitle className="text-base font-semibold flex flex-wrap items-center gap-2">
            <ChartNoAxesCombined className="h-5 w-5 text-primary" />
            <span>
              #{run.id} {run.label || run.strategy}
            </span>
            <Badge variant="secondary" className="text-[11px]">
              {KIND_LABELS[run.kind] ?? run.kind}
            </Badge>
          </CardTitle>
          <CardDescription className="text-xs flex flex-wrap gap-x-4 gap-y-1 font-mono">
            <span>{run.pair}</span>
            <span>{run.strategy}</span>
            <span>
              {day(run.fromTs)} 〜 {day(run.toTs)}
            </span>
            {cfg.execPeriod && <span>執行足 {cfg.execPeriod}</span>}
            {costs && (
              <span>
                スプレッド {costs.spread_pips} / スリッページ {costs.slippage_pips} pips · 手数料 {costs.commission_per_lot}/lot
              </span>
            )}
            {cfg.config && (
              <span>
                初期残高 {money(cfg.config.initialBalance)} ({cfg.config.accountCurrency})
              </span>
            )}
            <span>実行 {run.createdAt}</span>
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {run.kind === "sweep" && (
            <p className="text-xs text-muted-foreground">最良の組み合わせ（{formatParams(sweepRows[0]?.params) || "—"}）の成績</p>
          )}
          {run.kind === "walk_forward" && (
            <p className="text-xs text-muted-foreground">各窓のテスト期間（学習に使っていない期間）をつないだ成績</p>
          )}
          <MetricsSummary metrics={run.metrics} money={money} />
        </CardContent>
      </Card>

      {equity.length > 0 && (
        <Card className="shadow-xs">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-semibold">資産曲線</CardTitle>
          </CardHeader>
          <CardContent>
            <EquityChart points={equity} format={(v) => money(v)} />
          </CardContent>
        </Card>
      )}

      {run.kind === "sweep" && (
        <Card className="shadow-xs">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-semibold">パラメータの組み合わせ（{sweepRows.length} 通り・良い順）</CardTitle>
            <CardDescription className="text-xs">同じ期間で選んだ最良の組み合わせは過剰適合しやすいので、ウォークフォワードで確かめてください</CardDescription>
          </CardHeader>
          <CardContent>
            <SweepTable rows={sweepRows} money={money} />
          </CardContent>
        </Card>
      )}

      {run.kind === "walk_forward" && (
        <Card className="shadow-xs">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-semibold">ウォークフォワードの窓（{windows.length}）</CardTitle>
            <CardDescription className="text-xs">学習期間で最良のパラメータを選び、直後のテスト期間で検証します</CardDescription>
          </CardHeader>
          <CardContent>
            <WindowsTable windows={windows} money={money} />
          </CardContent>
        </Card>
      )}

      {run.kind !== "sweep" && (
        <div className="grid gap-4 lg:grid-cols-2">
          <Card className="shadow-xs">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-semibold">月別</CardTitle>
            </CardHeader>
            <CardContent>
              <MonthlyTable metrics={run.metrics} money={money} />
            </CardContent>
          </Card>
          <Card className="shadow-xs">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-semibold">決済理由とシグナル</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-1.5">
                <p className="text-xs font-medium">決済理由</p>
                <CountList counts={run.metrics.exitReasons} labels={EXIT_REASON_LABELS} />
              </div>
              {signals && (
                <>
                  <div className="flex flex-wrap gap-1.5">
                    <Badge variant="outline" className="text-[11px]">新規の意図 {signals.intents}</Badge>
                    <Badge variant="outline" className="text-[11px]">発注 {signals.placed}</Badge>
                    <Badge variant="outline" className="text-[11px] border-warning/40 text-warning">
                      ガードで却下 {signals.rejectedTotal}
                    </Badge>
                    <Badge variant="outline" className="text-[11px]">決済 {signals.closes}</Badge>
                    <Badge variant="outline" className="text-[11px]">取消 {signals.cancels}</Badge>
                    <Badge variant="outline" className="text-[11px]">変更 {signals.modifies}</Badge>
                    {cfg.decisions !== undefined && (
                      <Badge variant="outline" className="text-[11px]">判断 {cfg.decisions}</Badge>
                    )}
                  </div>
                  <div className="space-y-1.5">
                    <p className="text-xs font-medium">ガードで却下した理由（1件が複数に掛かることがあります）</p>
                    <CountList counts={signals.rejected} />
                  </div>
                </>
              )}
            </CardContent>
          </Card>
        </div>
      )}

      {trades.length > 0 && (
        <Card className="shadow-xs">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-semibold">取引（{trades.length} 件）</CardTitle>
          </CardHeader>
          <CardContent>
            <TradesTable key={run.id} trades={trades} pair={run.pair} money={money} />
          </CardContent>
        </Card>
      )}
    </div>
  );
}
