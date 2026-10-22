"use client";

import * as React from "react";
import type { EquityPoint } from "@/types/trading";

interface EquityChartProps {
  points: EquityPoint[];
  /** 金額の表示（口座通貨） */
  format: (v: number) => string;
}

const HEIGHT = 260;
const PAD = { top: 8, right: 8, bottom: 22, left: 80 };
const Y_TICKS = 4;

const dateLabel = (ms: number, withTime: boolean) =>
  new Date(ms).toLocaleString("ja-JP", {
    timeZone: "UTC",
    year: withTime ? undefined : "2-digit",
    month: "numeric",
    day: "numeric",
    ...(withTime ? { hour: "2-digit", minute: "2-digit" } : {}),
  });

interface Point {
  t: number;
  balance: number;
  equity: number;
}

/** バックテストの資産曲線。有効証拠金（含み損益込み）を実線、残高を階段の破線で描く */
export function EquityChart({ points, format }: EquityChartProps) {
  const [width, setWidth] = React.useState(0);
  const [hover, setHover] = React.useState<number | null>(null);
  const boxRef = React.useRef<HTMLDivElement>(null);

  // 線幅や文字を伸ばさないよう、実際の幅で描く
  React.useEffect(() => {
    const el = boxRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const data = React.useMemo<Point[]>(
    () =>
      points.flatMap((p) => {
        const t = Date.parse(p.time);
        return Number.isNaN(t) ? [] : [{ t, balance: p.balance, equity: p.equity }];
      }),
    [points]
  );
  const plotW = Math.max(0, width - PAD.left - PAD.right);
  const plotH = HEIGHT - PAD.top - PAD.bottom;

  const scale = React.useMemo(() => {
    if (data.length === 0) return null;
    const t0 = data[0].t;
    const t1 = data[data.length - 1].t;
    const values = data.flatMap((p) => [p.balance, p.equity]);
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

  const hovered = hover !== null ? data[hover] : null;
  const spansDays = scale ? scale.t1 - scale.t0 > 2 * 86400 * 1000 : false;
  // カーソル位置までの有効証拠金の最高値（ドローダウンの基準）
  const peak = React.useMemo(() => {
    if (hover === null) return null;
    let m = -Infinity;
    for (let i = 0; i <= hover; i++) m = Math.max(m, data[i].equity);
    return m;
  }, [data, hover]);

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
    <div className="space-y-2">
      <div className="flex items-center gap-4 text-[11px] text-muted-foreground">
        <span className="flex items-center gap-1.5">
          <svg width="16" height="4" aria-hidden>
            <line x1="0" x2="16" y1="2" y2="2" className="stroke-primary" strokeWidth={2} />
          </svg>
          有効証拠金（含み損益込み）
        </span>
        <span className="flex items-center gap-1.5">
          <svg width="16" height="4" aria-hidden>
            <line x1="0" x2="16" y1="2" y2="2" className="stroke-muted-foreground" strokeWidth={1.5} strokeDasharray="3 2" />
          </svg>
          残高
        </span>
      </div>
      <div ref={boxRef} className="relative" style={{ height: HEIGHT }}>
        {data.length === 0 && (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">資産曲線のデータがありません</div>
        )}
        {scale && width > 0 && (
          <svg width={width} height={HEIGHT} className="block" role="img" aria-label="資産曲線">
            {/* 目盛り */}
            {Array.from({ length: Y_TICKS + 1 }, (_, i) => {
              const v = scale.lo + ((scale.hi - scale.lo) * i) / Y_TICKS;
              const y = scale.y(v);
              return (
                <g key={i}>
                  <line x1={PAD.left} x2={width - PAD.right} y1={y} y2={y} className="stroke-border" strokeWidth={1} />
                  <text x={PAD.left - 8} y={y} dy="0.32em" textAnchor="end" className="fill-muted-foreground text-[10px] tabular-nums">
                    {format(v)}
                  </text>
                </g>
              );
            })}
            {timeTicks(scale.t0, scale.t1, plotW).map((t) => (
              <text key={t} x={scale.x(t)} y={HEIGHT - 6} textAnchor="middle" className="fill-muted-foreground text-[10px] tabular-nums">
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
              className="stroke-muted-foreground"
              strokeWidth={1.5}
              strokeDasharray="4 3"
            />
            <path
              d={data.map((p, i) => `${i === 0 ? "M" : "L"}${scale.x(p.t)},${scale.y(p.equity)}`).join("")}
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
                <circle cx={scale.x(hovered.t)} cy={scale.y(hovered.equity)} r={4} className="fill-primary stroke-card" strokeWidth={2} />
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

        {hovered && scale && (
          <div
            className="pointer-events-none absolute top-1 z-10 min-w-40 rounded-md border bg-popover px-3 py-2 text-xs text-popover-foreground shadow-sm"
            style={
              scale.x(hovered.t) > width / 2 ? { right: width - scale.x(hovered.t) + 12 } : { left: scale.x(hovered.t) + 12 }
            }
          >
            <div className="mb-1 text-muted-foreground tabular-nums">{dateLabel(hovered.t, true)} UTC</div>
            <div className="flex items-center justify-between gap-4">
              <span className="text-muted-foreground">有効証拠金</span>
              <span className="tabular-nums">{format(hovered.equity)}</span>
            </div>
            <div className="flex items-center justify-between gap-4">
              <span className="text-muted-foreground">残高</span>
              <span className="tabular-nums">{format(hovered.balance)}</span>
            </div>
            {peak !== null && peak > hovered.equity && (
              <div className="flex items-center justify-between gap-4">
                <span className="text-muted-foreground">高値から</span>
                <span className="tabular-nums text-loss">{format(hovered.equity - peak)}</span>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

/** 横軸の目盛り。幅に応じて 2〜6 本を等間隔に置く */
function timeTicks(t0: number, t1: number, plotW: number): number[] {
  if (t1 <= t0) return [t0];
  const n = Math.max(2, Math.min(6, Math.floor(plotW / 110)));
  return Array.from({ length: n }, (_, i) => t0 + ((t1 - t0) * i) / (n - 1));
}
