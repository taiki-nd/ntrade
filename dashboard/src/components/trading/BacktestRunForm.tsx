"use client";

import * as React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Loader2, Play } from "lucide-react";
import { defaultParams, TIMEFRAME_LABELS } from "@/lib/strategy";
import {
  STRATEGY_TIMEFRAMES,
  type BacktestRunRequest,
  type BarCoverage,
  type StrategyInfo,
  type StrategyTimeframe,
} from "@/types/trading";

interface BacktestRunFormProps {
  /** 取引設定の対象銘柄（候補。ほかの銘柄も入力できる） */
  pairs: string[];
  strategies: StrategyInfo[];
  coverage: BarCoverage[];
  running: boolean;
  onRun: (req: BacktestRunRequest) => void;
}

const TF_ITEMS = Object.fromEntries(STRATEGY_TIMEFRAMES.map((tf) => [tf, TIMEFRAME_LABELS[tf]]));
const EXEC_TF_ITEMS: Record<string, string> = { "": "既定（設定ファイル）", ...TF_ITEMS };

const isoDay = (d: Date) => d.toISOString().slice(0, 10);

/** 空欄は undefined（設定ファイルの値を使う） */
const optionalNumber = (v: string): number | undefined => {
  const n = Number(v);
  return v.trim() !== "" && Number.isFinite(n) ? n : undefined;
};

/** 単発バックテストの実行フォームと、DB にある足の範囲 */
export function BacktestRunForm({ pairs, strategies, coverage, running, onRun }: BacktestRunFormProps) {
  const [pair, setPair] = React.useState("");
  const [strategyName, setStrategyName] = React.useState("");
  const [timeframe, setTimeframe] = React.useState<StrategyTimeframe>("M5");
  // 入力中の値をそのまま持つので文字列（空欄は戦略の既定値）
  const [params, setParams] = React.useState<Record<string, string>>({});
  const [from, setFrom] = React.useState(() => isoDay(new Date(Date.now() - 365 * 86400 * 1000)));
  const [to, setTo] = React.useState(() => isoDay(new Date()));
  const [execTimeframe, setExecTimeframe] = React.useState<"" | StrategyTimeframe>("");
  const [initialBalance, setInitialBalance] = React.useState("");
  const [spreadPips, setSpreadPips] = React.useState("");
  const [commissionPerLot, setCommissionPerLot] = React.useState("");
  const [label, setLabel] = React.useState("");

  // 未選択の間は候補の先頭を使う
  const effectivePair = pair || pairs[0] || "";
  const info = strategies.find((s) => s.name === (strategyName || strategies[0]?.name));

  const selectStrategy = (name: string) => {
    setStrategyName(name);
    const next = strategies.find((s) => s.name === name);
    setParams(Object.fromEntries(Object.entries(defaultParams(next)).map(([k, v]) => [k, String(v)])));
  };

  const paramValue = (name: string) => params[name] ?? String(info?.params.find((p) => p.name === name)?.default ?? "");

  const coverageRows = [...coverage].sort((a, b) => a.pair.localeCompare(b.pair) || periodOrder(a.period) - periodOrder(b.period));
  const pairOptions = Array.from(new Set([...pairs, ...coverage.map((c) => c.pair)]));

  const valid = !!effectivePair.trim() && !!info && !!from && !!to && from < to;

  const submit = () => {
    if (!valid || !info) return;
    onRun({
      pair: effectivePair.trim(),
      from: `${from}T00:00:00Z`,
      // 終了日はその日の終わりまで含める
      to: `${to}T23:59:59Z`,
      strategy: {
        name: info.name,
        timeframe,
        params: Object.fromEntries(
          info.params.flatMap((p) => {
            const v = optionalNumber(paramValue(p.name));
            return v === undefined ? [] : [[p.name, v]];
          })
        ),
      },
      initialBalance: optionalNumber(initialBalance),
      spreadPips: optionalNumber(spreadPips),
      commissionPerLot: optionalNumber(commissionPerLot),
      execTimeframe: execTimeframe || undefined,
      label: label.trim(),
    });
  };

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1.5">
          <Label htmlFor="bt-pair" className="text-xs text-muted-foreground">
            銘柄
          </Label>
          <Input
            id="bt-pair"
            list="bt-pair-options"
            value={pair}
            placeholder={pairs[0] ?? "USDJPY_z"}
            onChange={(e) => setPair(e.target.value)}
            className="h-8 text-xs font-mono"
          />
          <datalist id="bt-pair-options">
            {pairOptions.map((p) => (
              <option key={p} value={p} />
            ))}
          </datalist>
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">ラベル</Label>
          <Input value={label} placeholder="任意のメモ" onChange={(e) => setLabel(e.target.value)} className="h-8 text-xs" />
        </div>

        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">戦略</Label>
          <Select
            value={info?.name ?? ""}
            items={Object.fromEntries(strategies.map((s) => [s.name, s.title]))}
            onValueChange={(v) => selectStrategy(String(v ?? ""))}
          >
            <SelectTrigger size="sm" className="h-8 w-full text-xs">
              <SelectValue placeholder="読み込み中..." />
            </SelectTrigger>
            <SelectContent>
              {strategies.map((s) => (
                <SelectItem key={s.name} value={s.name}>
                  {s.title}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">時間足</Label>
          <Select value={timeframe} items={TF_ITEMS} onValueChange={(v) => setTimeframe(v as StrategyTimeframe)}>
            <SelectTrigger size="sm" className="h-8 w-full text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {STRATEGY_TIMEFRAMES.map((tf) => (
                <SelectItem key={tf} value={tf}>
                  {TIMEFRAME_LABELS[tf]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      {info && (
        <div className="space-y-2 rounded-lg border bg-muted/30 p-3">
          <p className="text-xs text-muted-foreground">{info.description}</p>
          <div className="grid grid-cols-2 gap-3">
            {info.params.map((p) => (
              <div key={p.name} className="space-y-1">
                <Label className="text-xs font-mono" title={p.description}>
                  {p.name}
                </Label>
                <Input
                  type="number"
                  step={p.integer ? "1" : "any"}
                  min={p.min}
                  max={p.max}
                  value={paramValue(p.name)}
                  placeholder={String(p.default)}
                  onChange={(e) => setParams({ ...params, [p.name]: e.target.value })}
                  className="h-8 text-xs font-mono"
                />
                <p className="text-[11px] text-muted-foreground" title={p.description}>
                  {p.min}〜{p.max}（既定 {p.default}）{p.description && ` ${p.description}`}
                </p>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1.5">
          <Label htmlFor="bt-from" className="text-xs text-muted-foreground">
            開始日 (UTC)
          </Label>
          <Input id="bt-from" type="date" value={from} onChange={(e) => setFrom(e.target.value)} className="h-8 text-xs" />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="bt-to" className="text-xs text-muted-foreground">
            終了日 (UTC・その日を含む)
          </Label>
          <Input id="bt-to" type="date" value={to} onChange={(e) => setTo(e.target.value)} className="h-8 text-xs" />
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">初期残高</Label>
          <Input
            type="number"
            value={initialBalance}
            placeholder="設定ファイルの値"
            onChange={(e) => setInitialBalance(e.target.value)}
            className="h-8 text-xs font-mono"
          />
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">執行の時間足</Label>
          <Select
            value={execTimeframe}
            items={EXEC_TF_ITEMS}
            onValueChange={(v) => setExecTimeframe((v ?? "") as "" | StrategyTimeframe)}
          >
            <SelectTrigger size="sm" className="h-8 w-full text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {Object.entries(EXEC_TF_ITEMS).map(([v, l]) => (
                <SelectItem key={v} value={v}>
                  {l}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">スプレッド (pips)</Label>
          <Input
            type="number"
            step="any"
            value={spreadPips}
            placeholder="設定ファイルの値"
            onChange={(e) => setSpreadPips(e.target.value)}
            className="h-8 text-xs font-mono"
          />
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">手数料 (1 lot・片道)</Label>
          <Input
            type="number"
            step="any"
            value={commissionPerLot}
            placeholder="設定ファイルの値"
            onChange={(e) => setCommissionPerLot(e.target.value)}
            className="h-8 text-xs font-mono"
          />
        </div>
      </div>
      <p className="text-[11px] text-muted-foreground">
        空欄の項目は <code className="font-mono">config/backtest.toml</code> の値を使います。ガード（リスク設定）は取引設定と同じもの（guard = &quot;settings&quot;）が当たります。
      </p>

      <Button onClick={submit} disabled={!valid || running} className="w-full h-9 gap-1.5 text-sm">
        {running ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
        {running ? "実行中…（期間とデータ量によって 1 分ほどかかります）" : "バックテストを実行"}
      </Button>

      <Separator />

      <div className="space-y-2">
        <div>
          <h3 className="text-sm font-medium">ヒストリカルデータの範囲</h3>
          <p className="text-[11px] text-muted-foreground">DB にある足。この範囲の中でだけ検証できます（行をクリックすると銘柄と期間を入れます）</p>
        </div>
        {coverageRows.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            足のデータがありません。<code className="font-mono">cargo run --release --bin backtest -- fetch --pair ... --from ...</code> で取得してください
          </p>
        ) : (
          <div className="max-h-64 overflow-auto rounded-md border">
            <Table>
              <TableHeader className="bg-muted/40">
                <TableRow>
                  <TableHead className="text-xs">銘柄</TableHead>
                  <TableHead className="text-xs">足</TableHead>
                  <TableHead className="text-xs text-right">本数</TableHead>
                  <TableHead className="text-xs">期間 (UTC)</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {coverageRows.map((c) => (
                  <TableRow
                    key={`${c.pair}-${c.period}`}
                    className="cursor-pointer"
                    onClick={() => {
                      setPair(c.pair);
                      if (c.first) setFrom(c.first.slice(0, 10));
                      if (c.last) setTo(c.last.slice(0, 10));
                    }}
                  >
                    <TableCell className="text-xs font-mono">{c.pair}</TableCell>
                    <TableCell className="text-xs font-mono">{c.period}</TableCell>
                    <TableCell className="text-xs text-right tabular-nums">{c.count.toLocaleString()}</TableCell>
                    <TableCell className="text-xs font-mono whitespace-nowrap">
                      {c.first?.slice(0, 10) ?? "—"} 〜 {c.last?.slice(0, 10) ?? "—"}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </div>
    </div>
  );
}

const PERIOD_ORDER = ["1M", "5M", "15M", "30M", "1H", "4H", "1D"];
function periodOrder(p: string): number {
  const i = PERIOD_ORDER.indexOf(p);
  return i < 0 ? PERIOD_ORDER.length : i;
}
