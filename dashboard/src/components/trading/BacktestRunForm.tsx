"use client";

import * as React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Loader2, Play } from "lucide-react";
import { defaultParams, TIMEFRAME_LABELS } from "@/lib/strategy";
import {
  STRATEGY_TIMEFRAMES,
  type BacktestJobRequest,
  type BacktestRunRequest,
  type Objective,
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
  /** 単発（終わるまで待つ） */
  onRun: (req: BacktestRunRequest) => void;
  /** グリッドサーチ・ウォークフォワード（ジョブとして順番待ちに入れる） */
  onSubmitJob: (req: BacktestJobRequest) => void;
}

type Mode = "single" | "sweep" | "walk_forward";

const OBJECTIVES: Record<Objective, string> = {
  profit_factor: "プロフィットファクター",
  net_profit: "純損益",
  sharpe: "シャープレシオ",
  recovery_factor: "リカバリーファクター",
  expectancy: "期待値（1 取引）",
};

/** "10,20,30" や "10..30:5" は探索する値、数値 1 つは固定値 */
const isGridValue = (v: string) => v.includes(",") || v.includes("..");

const TF_ITEMS = Object.fromEntries(STRATEGY_TIMEFRAMES.map((tf) => [tf, TIMEFRAME_LABELS[tf]]));
const EXEC_TF_ITEMS: Record<string, string> = { "": "既定（設定ファイル）", ...TF_ITEMS };

const isoDay = (d: Date) => d.toISOString().slice(0, 10);

/** 空欄は undefined（設定ファイルの値を使う） */
const optionalNumber = (v: string): number | undefined => {
  const n = Number(v);
  return v.trim() !== "" && Number.isFinite(n) ? n : undefined;
};

/** バックテストの実行フォーム（単発・グリッドサーチ・ウォークフォワード）と、DB にある足の範囲 */
export function BacktestRunForm({ pairs, strategies, coverage, running, onRun, onSubmitJob }: BacktestRunFormProps) {
  const [mode, setMode] = React.useState<Mode>("single");
  const [objective, setObjective] = React.useState<Objective>("profit_factor");
  const [minTrades, setMinTrades] = React.useState("30");
  const [trainDays, setTrainDays] = React.useState("180");
  const [testDays, setTestDays] = React.useState("60");
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

  const searching = mode !== "single";
  const grid = info
    ? info.params.flatMap((p) => {
        const v = paramValue(p.name).replace(/\s+/g, "");
        return searching && isGridValue(v) ? [`${p.name}=${v}`] : [];
      })
    : [];
  const valid =
    !!effectivePair.trim() &&
    !!info &&
    !!from &&
    !!to &&
    from < to &&
    (!searching || grid.length > 0) &&
    (mode !== "walk_forward" || (Number(trainDays) > 0 && Number(testDays) > 0));

  const submit = () => {
    if (!valid || !info) return;
    const base: BacktestRunRequest = {
      pair: effectivePair.trim(),
      from: `${from}T00:00:00Z`,
      // 終了日はその日の終わりまで含める
      to: `${to}T23:59:59Z`,
      strategy: {
        name: info.name,
        timeframe,
        // 探索するパラメータは grid で渡すので、ここには固定値だけを入れる
        params: Object.fromEntries(
          info.params.flatMap((p) => {
            const raw = paramValue(p.name);
            const v = searching && isGridValue(raw) ? undefined : optionalNumber(raw);
            return v === undefined ? [] : [[p.name, v]];
          })
        ),
      },
      initialBalance: optionalNumber(initialBalance),
      spreadPips: optionalNumber(spreadPips),
      commissionPerLot: optionalNumber(commissionPerLot),
      execTimeframe: execTimeframe || undefined,
      label: label.trim(),
    };
    if (mode === "single") {
      onRun(base);
      return;
    }
    const gridReq = { ...base, grid, objective, minTrades: Math.max(1, Math.round(Number(minTrades) || 1)) };
    onSubmitJob(
      mode === "sweep"
        ? { kind: "sweep", ...gridReq }
        : { kind: "walk_forward", ...gridReq, trainDays: Math.round(Number(trainDays)), testDays: Math.round(Number(testDays)) }
    );
  };

  return (
    <div className="space-y-4">
      <Tabs value={mode} onValueChange={(v) => setMode(v as Mode)}>
        <TabsList className="w-full">
          <TabsTrigger value="single" className="text-xs">単発</TabsTrigger>
          <TabsTrigger value="sweep" className="text-xs">グリッドサーチ</TabsTrigger>
          <TabsTrigger value="walk_forward" className="text-xs">ウォークフォワード</TabsTrigger>
        </TabsList>
      </Tabs>
      <p className="text-[11px] text-muted-foreground">
        {mode === "single" && "1 つのパラメータで期間全体を走らせます。"}
        {mode === "sweep" && "パラメータの候補の組み合わせを全部走らせて、目的の指標が高い順に並べます。過去に合わせ込んだ結果なので、採用はウォークフォワードで確かめてください。"}
        {mode === "walk_forward" &&
          "学習期間で最良の組み合わせを選び、直後の検証期間で走らせる、を期間をずらしながら繰り返します。検証期間だけをつないだ成績が、未知のデータでの成績の目安です。"}
      </p>
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
          {searching && (
            <p className="text-[11px] text-muted-foreground">
              探索するパラメータは <code className="font-mono">10,20,30</code>（候補の一覧）か <code className="font-mono">10..30:5</code>（開始..終了:刻み）で書きます。数値 1 つなら固定です。
            </p>
          )}
          <div className="grid grid-cols-2 gap-3">
            {info.params.map((p) => (
              <div key={p.name} className="space-y-1">
                <Label className="text-xs font-mono" title={p.description}>
                  {p.name}
                </Label>
                <Input
                  type={searching ? "text" : "number"}
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

      {searching && (
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1.5">
            <Label className="text-xs text-muted-foreground">最大化する指標</Label>
            <Select value={objective} items={OBJECTIVES} onValueChange={(v) => setObjective(v as Objective)}>
              <SelectTrigger size="sm" className="h-8 w-full text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {Object.entries(OBJECTIVES).map(([v, l]) => (
                  <SelectItem key={v} value={v}>
                    {l}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1.5">
            <Label className="text-xs text-muted-foreground">最低取引数</Label>
            <Input type="number" min={1} value={minTrades} onChange={(e) => setMinTrades(e.target.value)} className="h-8 text-xs font-mono" />
          </div>
          {mode === "walk_forward" && (
            <>
              <div className="space-y-1.5">
                <Label className="text-xs text-muted-foreground">学習期間（日）</Label>
                <Input type="number" min={1} value={trainDays} onChange={(e) => setTrainDays(e.target.value)} className="h-8 text-xs font-mono" />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs text-muted-foreground">検証期間（日）</Label>
                <Input type="number" min={1} value={testDays} onChange={(e) => setTestDays(e.target.value)} className="h-8 text-xs font-mono" />
              </div>
            </>
          )}
          <p className="col-span-2 text-[11px] text-muted-foreground">
            {grid.length > 0
              ? `探索: ${grid.join("  ")}（取引数が最低取引数に満たない組み合わせは選びません）`
              : "探索するパラメータがありません。上のパラメータのどれかに候補を書いてください"}
          </p>
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
        {running
          ? "実行中…（期間とデータ量によって 1 分ほどかかります）"
          : mode === "single"
            ? "バックテストを実行"
            : "ジョブとして実行（進み具合は一覧の上に出ます）"}
      </Button>

      <Separator />

      <div className="space-y-2">
        <div>
          <h3 className="text-sm font-medium">ヒストリカルデータの範囲</h3>
          <p className="text-[11px] text-muted-foreground">DB にある足。この範囲の中でだけ検証できます（行をクリックすると銘柄と期間を入れます）</p>
        </div>
        {coverageRows.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            足のデータがありません。バックテスト画面の「データを取得」から取得してください
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
