"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { LineChart, LayoutGrid, Maximize2 } from "lucide-react";

interface Candle {
  time: string;
  open: number;
  high: number;
  low: number;
  close: number;
  isPinbar?: boolean;
}

// USD/JPY 5分足の直近モックローソク足データ
const m5Candles: Candle[] = [
  { time: "06:55", open: 154.28, high: 154.32, low: 154.22, close: 154.24 },
  { time: "07:00", open: 154.24, high: 154.26, low: 154.18, close: 154.19 },
  { time: "07:05", open: 154.19, high: 154.22, low: 154.14, close: 154.16 },
  { time: "07:10", open: 154.16, high: 154.18, low: 154.10, close: 154.12 },
  { time: "07:15", open: 154.12, high: 154.15, low: 154.09, close: 154.14 },
  { time: "07:20", open: 154.14, high: 154.18, low: 154.11, close: 154.13 },
  { time: "07:25", open: 154.13, high: 154.16, low: 154.08, close: 154.10 },
  // ピンバー（下ヒゲ68%）
  { time: "07:30", open: 154.11, high: 154.22, low: 154.08, close: 154.21, isPinbar: true },
  { time: "07:35", open: 154.21, high: 154.39, low: 154.20, close: 154.38 },
];

export function ChartPreview() {
  const [selectedPair, setSelectedPair] = React.useState<"USDJPY" | "EURUSD">("USDJPY");

  // SVGミニチャートレンダラー
  const renderCandleSvg = (candles: Candle[], label: string, supportLevel: number) => {
    const minVal = Math.min(...candles.map((c) => c.low)) - 0.05;
    const maxVal = Math.max(...candles.map((c) => c.high)) + 0.05;
    const range = maxVal - minVal;

    const width = 360;
    const height = 160;
    const paddingX = 25;
    const paddingY = 20;

    const getY = (val: number) => {
      return height - paddingY - ((val - minVal) / range) * (height - paddingY * 2);
    };

    const candleWidth = 14;
    const gap = (width - paddingX * 2) / (candles.length - 1);

    const supportY = getY(supportLevel);

    return (
      <div className="relative rounded-lg border bg-muted/20 p-2.5 overflow-hidden">
        <div className="flex items-center justify-between mb-1.5 px-1">
          <div className="flex items-center gap-2">
            <span className="text-xs font-bold font-mono">{label}</span>
            <span className="text-[10px] text-muted-foreground">EMA20/50 + サポレジ</span>
          </div>
          {label === "5M (精密トリガー)" && (
            <Badge variant="outline" className="text-[10px] h-4 bg-emerald-50 text-emerald-700 border-emerald-300 dark:bg-emerald-950/50 dark:text-emerald-400">
              ピンバー検出
            </Badge>
          )}
        </div>

        <svg viewBox={`0 0 ${width} ${height}`} className="w-full h-36">
          {/* グリッド背景線 */}
          <line x1={paddingX} y1={paddingY} x2={width - paddingX} y2={paddingY} stroke="currentColor" strokeOpacity={0.08} strokeDasharray="3 3" />
          <line x1={paddingX} y1={height / 2} x2={width - paddingX} y2={height / 2} stroke="currentColor" strokeOpacity={0.08} strokeDasharray="3 3" />
          <line x1={paddingX} y1={height - paddingY} x2={width - paddingX} y2={height - paddingY} stroke="currentColor" strokeOpacity={0.08} strokeDasharray="3 3" />

          {/* サポートライン (水平線) */}
          <line
            x1={paddingX}
            y1={supportY}
            x2={width - paddingX}
            y2={supportY}
            stroke="#10b981"
            strokeWidth={1.5}
            strokeDasharray="4 2"
          />
          <text x={width - paddingX - 45} y={supportY - 4} fill="#10b981" fontSize={9} fontWeight="bold">
            SUP 154.12
          </text>

          {/* ローソク足群 */}
          {candles.map((c, i) => {
            const x = paddingX + i * gap;
            const yOpen = getY(c.open);
            const yClose = getY(c.close);
            const yHigh = getY(c.high);
            const yLow = getY(c.low);
            const isBullish = c.close >= c.open;
            const color = isBullish ? "#10b981" : "#f43f5e";
            const topBody = Math.min(yOpen, yClose);
            const bodyHeight = Math.max(Math.abs(yOpen - yClose), 2);

            return (
              <g key={i}>
                {/* ヒゲ */}
                <line x1={x} y1={yHigh} x2={x} y2={yLow} stroke={color} strokeWidth={1.5} />
                {/* 実体 */}
                <rect
                  x={x - candleWidth / 2}
                  y={topBody}
                  width={candleWidth}
                  height={bodyHeight}
                  fill={color}
                  rx={1}
                />
                {/* ピンバーのハイライト枠 */}
                {c.isPinbar && (
                  <circle
                    cx={x}
                    cy={yLow}
                    r={6}
                    fill="none"
                    stroke="#f59e0b"
                    strokeWidth={1.5}
                    className="animate-pulse"
                  />
                )}
              </g>
            );
          })}
        </svg>
      </div>
    );
  };

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-row items-center justify-between pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <LineChart className="h-5 w-5 text-emerald-500" />
            <span>マルチタイムフレーム (MTF) チャートプレビュー</span>
          </CardTitle>
          <CardDescription className="text-xs">
            Rust (plotters) が推論時に自動生成する4分割チャートイメージ
          </CardDescription>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setSelectedPair("USDJPY")}
            className={`px-2.5 py-1 text-xs font-semibold rounded-md transition-colors ${
              selectedPair === "USDJPY"
                ? "bg-primary text-primary-foreground"
                : "bg-muted text-muted-foreground hover:bg-muted/80"
            }`}
          >
            USD/JPY
          </button>
          <button
            type="button"
            onClick={() => setSelectedPair("EURUSD")}
            className={`px-2.5 py-1 text-xs font-semibold rounded-md transition-colors ${
              selectedPair === "EURUSD"
                ? "bg-primary text-primary-foreground"
                : "bg-muted text-muted-foreground hover:bg-muted/80"
            }`}
          >
            EUR/USD
          </button>
        </div>
      </CardHeader>
      <CardContent>
        {/* 4分割グリッド表示 */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {renderCandleSvg(
            [
              { time: "00:00", open: 153.2, high: 153.8, low: 153.1, close: 153.7 },
              { time: "04:00", open: 153.7, high: 154.1, low: 153.6, close: 154.0 },
              { time: "08:00", open: 154.0, high: 154.5, low: 153.9, close: 154.38 },
            ],
            "4H (上位大局観・上昇ダウ)",
            153.8
          )}
          {renderCandleSvg(
            [
              { time: "03:00", open: 153.9, high: 154.2, low: 153.85, close: 154.1 },
              { time: "04:00", open: 154.1, high: 154.25, low: 154.05, close: 154.18 },
              { time: "05:00", open: 154.18, high: 154.3, low: 154.12, close: 154.25 },
              { time: "06:00", open: 154.25, high: 154.35, low: 154.15, close: 154.22 },
              { time: "07:00", open: 154.22, high: 154.4, low: 154.12, close: 154.38 },
            ],
            "1H (波とEMA20サポレジ)",
            154.12
          )}
          {renderCandleSvg(
            [
              { time: "06:30", open: 154.3, high: 154.35, low: 154.22, close: 154.24 },
              { time: "06:45", open: 154.24, high: 154.28, low: 154.18, close: 154.2 },
              { time: "07:00", open: 154.2, high: 154.22, low: 154.12, close: 154.15 },
              { time: "07:15", open: 154.15, high: 154.18, low: 154.08, close: 154.14 },
              { time: "07:30", open: 154.14, high: 154.4, low: 154.08, close: 154.38 },
            ],
            "15M (プルバック形成)",
            154.12
          )}
          {renderCandleSvg(m5Candles, "5M (精密トリガー)", 154.12)}
        </div>
      </CardContent>
    </Card>
  );
}
