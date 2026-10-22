"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { tradingApi } from "@/lib/trading-api";
import { parseEngineTime } from "@/lib/time";
import type { BalancePoint } from "@/types/trading";

interface BalanceChartProps {
  /** 親から再取得を促すための値（変わるたびに取り直す） */
  refreshKey?: number;
}

/** 表示期間（days を省略すると全期間） */
const RANGES = [
  { id: "1w", label: "1週間", days: 7 },
  { id: "1m", label: "1ヶ月", days: 30 },
  { id: "3m", label: "3ヶ月", days: 90 },
  { id: "1y", label: "1年", days: 365 },
  { id: "all", label: "全期間", days: undefined },
] as const;
type RangeId = (typeof RANGES)[number]["id"];

/** エンジンは 5 分ごとに cTrader の履歴を取り込む */
const POLL_INTERVAL = 60000;
const HEIGHT = 220;
const PAD = { top: 8, right: 8, bottom: 22, left: 72 };
const Y_TICKS = 4;

const yen = new Intl.NumberFormat("ja-JP", { style: "currency", currency: "JPY", maximumFractionDigits: 0 });
const signedYen = new Intl.NumberFormat("ja-JP", {
  style: "currency",
  currency: "JPY",
  maximumFractionDigits: 0,
  signDisplay: "exceptZero",
});
const dateLabel = (ms: number, withTime: boolean) =>
  new Date(ms).toLocaleString("ja-JP", {
    month: "numeric",
    day: "numeric",
    ...(withTime ? { hour: "2-digit", minute: "2-digit" } : {}),
  });

interface Point {
  t: number;
  balance: number;
}

/** 口座残高の推移。決済・入出金でしか動かないので階段状に描く */
export function BalanceChart({ refreshKey }: BalanceChartProps) {
  const [range, setRange] = React.useState<RangeId>("1m");
  const [points, setPoints] = React.useState<Point[] | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [width, setWidth] = React.useState(0);
  const [hover, setHover] = React.useState<number | null>(null);
  const boxRef = React.useRef<HTMLDivElement>(null);

  const load = React.useCallback(async () => {
    try {
      const res = await tradingApi.getBalanceHistory(RANGES.find((r) => r.id === range)?.days);
      if (!res.success || !res.data) {
        setError(res.message || "残高の推移を取得できませんでした。");
        return;
      }
      setError(null);
      setPoints(
        res.data.flatMap((p: BalancePoint) => {
          const t = parseEngineTime(p.time);
          return t === null ? [] : [{ t, balance: p.balance }];
        })
      );
    } catch (err) {
      // 推移 API の無い古いエンジンが動いている場合も 404 でここに来る
      setError(err instanceof Error ? err.message : "残高の推移を取得できませんでした。");
    }
  }, [range]);

  React.useEffect(() => {
    const initial = setTimeout(load, 0);
    const interval = setInterval(load, POLL_INTERVAL);
    return () => {
      clearTimeout(initial);
      clearInterval(interval);
    };
  }, [load, refreshKey]);

  // 線幅や文字を伸ばさないよう、実際の幅で描く
  React.useEffect(() => {
    const el = boxRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const data = React.useMemo(() => points ?? [], [points]);
  const plotW = Math.max(0, width - PAD.left - PAD.right);
  const plotH = HEIGHT - PAD.top - PAD.bottom;

  const scale = React.useMemo(() => {
    if (data.length === 0) return null;
    const t0 = data[0].t;
    const t1 = data[data.length - 1].t;
    const values = data.map((p) => p.balance);
    let lo = Math.min(...values);
    let hi = Math.max(...values);
    // 値が動いていなくても線が上下の端に張り付かないよう余白を取る
    const pad = hi > lo ? (hi - lo) * 0.08 : Math.max(Math.abs(hi) * 0.01, 1);
    lo -= pad;
    hi += pad;
    return {
      t0,
      t1,
      lo,
      hi,
      x: (t: number) => PAD.left + (t1 > t0 ? ((t - t0) / (t1 - t0)) * plotW : plotW / 2),
      y: (v: number) => PAD.top + (1 - (v - lo) / (hi - lo)) * plotH,
    };
  }, [data, plotW, plotH]);

  const first = data[0];
  const latest = data[data.length - 1];
  const hovered = hover !== null ? data[hover] : null;
  const spansDays = scale ? scale.t1 - scale.t0 > 2 * 86400 * 1000 : false;

  // カーソル位置で有効だった残高（その時刻以前の最後の点）
  const handleMove = (e: React.PointerEvent<SVGRectElement>) => {
    if (!scale || data.length === 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const t = scale.t0 + ((e.clientX - rect.left) / Math.max(1, rect.width)) * (scale.t1 - scale.t0);
    let lo = 0;
    let hi = data.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (data[mid].t <= t) lo = mid;
      else hi = mid - 1;
    }
    setHover(lo);
  };

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-4 space-y-0">
        <div className="space-y-1">
          <CardTitle>残高の推移</CardTitle>
          <CardDescription>cTrader の決済・入出金の履歴から描いています</CardDescription>
        </div>
        <Tabs value={range} onValueChange={(v) => setRange(v as RangeId)}>
          <TabsList>
            {RANGES.map((r) => (
              <TabsTrigger key={r.id} value={r.id} className="text-xs">
                {r.label}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </CardHeader>
      <CardContent className="space-y-3">
        {/* 現在の残高と期間中の増減 */}
        <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
          <span className="text-muted-foreground">
            現在 <span className="text-sm tabular-nums text-foreground">{latest ? yen.format(latest.balance) : "—"}</span>
          </span>
          {first && latest && (
            <span className="text-muted-foreground">
              期間中{" "}
              <span
                className={`tabular-nums ${
                  latest.balance > first.balance ? "text-profit" : latest.balance < first.balance ? "text-loss" : "text-foreground"
                }`}
              >
                {signedYen.format(latest.balance - first.balance)}
              </span>
            </span>
          )}
        </div>

        <div ref={boxRef} className="relative" style={{ height: HEIGHT }}>
          {error && points === null && (
            <div className="flex h-full flex-col items-center justify-center gap-1 text-center text-sm text-muted-foreground">
              <span>残高の推移を取得できませんでした。エンジンを再起動すると表示されます。</span>
              <span className="text-xs">{error}</span>
            </div>
          )}
          {points !== null && data.length === 0 && (
            <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
              この期間の履歴はまだありません。cTrader に接続すると、エンジンが履歴を取り込みます。
            </div>
          )}
          {scale && width > 0 && (
            <svg width={width} height={HEIGHT} className="block" role="img" aria-label="残高の推移">
              {/* 目盛り */}
              {Array.from({ length: Y_TICKS + 1 }, (_, i) => {
                const v = scale.lo + ((scale.hi - scale.lo) * i) / Y_TICKS;
                const y = scale.y(v);
                return (
                  <g key={i}>
                    <line x1={PAD.left} x2={width - PAD.right} y1={y} y2={y} className="stroke-border" strokeWidth={1} />
                    <text x={PAD.left - 8} y={y} dy="0.32em" textAnchor="end" className="fill-muted-foreground text-[10px] tabular-nums">
                      {yen.format(v)}
                    </text>
                  </g>
                );
              })}
              {timeTicks(scale.t0, scale.t1, plotW).map((t) => (
                <text
                  key={t}
                  x={scale.x(t)}
                  y={HEIGHT - 6}
                  textAnchor="middle"
                  className="fill-muted-foreground text-[10px] tabular-nums"
                >
                  {dateLabel(t, !spansDays)}
                </text>
              ))}

              <path
                d={data
                  .map((p, i) =>
                    i === 0 ? `M${scale.x(p.t)},${scale.y(p.balance)}` : `H${scale.x(p.t)}V${scale.y(p.balance)}`
                  )
                  .join("")}
                fill="none"
                className="stroke-primary"
                strokeWidth={2}
                strokeLinejoin="round"
                strokeLinecap="round"
              />

              {/* ホバー: 縦線と点 */}
              {hovered && (
                <g pointerEvents="none">
                  <line
                    x1={scale.x(hovered.t)}
                    x2={scale.x(hovered.t)}
                    y1={PAD.top}
                    y2={PAD.top + plotH}
                    className="stroke-muted-foreground"
                    strokeWidth={1}
                    strokeDasharray="3 3"
                  />
                  <circle cx={scale.x(hovered.t)} cy={scale.y(hovered.balance)} r={4} className="fill-primary stroke-card" strokeWidth={2} />
                </g>
              )}
              <rect
                x={PAD.left}
                y={PAD.top}
                width={plotW}
                height={plotH}
                fill="transparent"
                onPointerMove={handleMove}
                onPointerLeave={() => setHover(null)}
              />
            </svg>
          )}

          {hovered && hover !== null && scale && (
            <div
              className="pointer-events-none absolute top-1 z-10 min-w-36 rounded-md border bg-popover px-3 py-2 text-xs text-popover-foreground shadow-sm"
              style={
                scale.x(hovered.t) > width / 2
                  ? { right: width - scale.x(hovered.t) + 12 }
                  : { left: scale.x(hovered.t) + 12 }
              }
            >
              <div className="mb-1 text-muted-foreground tabular-nums">{dateLabel(hovered.t, true)}</div>
              <div className="flex items-center justify-between gap-4">
                <span className="text-muted-foreground">残高</span>
                <span className="tabular-nums">{yen.format(hovered.balance)}</span>
              </div>
              {hover > 0 && (
                <div className="flex items-center justify-between gap-4">
                  <span className="text-muted-foreground">増減</span>
                  <span className="tabular-nums">{signedYen.format(hovered.balance - data[hover - 1].balance)}</span>
                </div>
              )}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

/** 横軸の目盛り。幅に応じて 2〜6 本を等間隔に置く */
function timeTicks(t0: number, t1: number, plotW: number): number[] {
  if (t1 <= t0) return [t0];
  const n = Math.max(2, Math.min(6, Math.floor(plotW / 110)));
  return Array.from({ length: n }, (_, i) => t0 + ((t1 - t0) * i) / (n - 1));
}
