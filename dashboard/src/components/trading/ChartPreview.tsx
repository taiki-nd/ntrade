"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { LineChart, RefreshCw, Sparkles, Image as ImageIcon, LayoutGrid, Loader2 } from "lucide-react";
import { tradingApi } from "@/lib/trading-api";
import { toast } from "sonner";

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
  { time: "07:30", open: 154.11, high: 154.22, low: 154.08, close: 154.21, isPinbar: true },
  { time: "07:35", open: 154.21, high: 154.39, low: 154.20, close: 154.38 },
];

export function ChartPreview() {
  const [selectedPair, setSelectedPair] = React.useState<"USDJPY" | "EURUSD">("USDJPY");
  const [chartMode, setChartMode] = React.useState<"png" | "svg">("png");
  const [chartUrl, setChartUrl] = React.useState<string>(tradingApi.getLatestChartUrl());
  const [isRegenerating, setIsRegenerating] = React.useState<boolean>(false);
  const [imageError, setImageError] = React.useState<boolean>(false);

  // チャート画像の再生成
  const handleRegenerate = async () => {
    try {
      setIsRegenerating(true);
      setImageError(false);
      const res = await tradingApi.generateChart();
      if (res.success) {
        setChartUrl(tradingApi.getLatestChartUrl());
        toast.success("4分割チャートPNGを再生成しました", {
          description: "4H/1H/15M/5M のEMA・サポレジ・トリガーが更新されました。",
        });
      } else {
        toast.error("チャートの再生成に失敗しました", {
          description: res.message,
        });
      }
    } catch (err: unknown) {
      toast.error("Rustエンジンへの接続に失敗しました", {
        description: err instanceof Error ? err.message : "エンジンが起動しているか確認してください。",
      });
    } finally {
      setIsRegenerating(false);
    }
  };

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
          <line x1={paddingX} y1={paddingY} x2={width - paddingX} y2={paddingY} stroke="currentColor" strokeOpacity={0.08} strokeDasharray="3 3" />
          <line x1={paddingX} y1={height / 2} x2={width - paddingX} y2={height / 2} stroke="currentColor" strokeOpacity={0.08} strokeDasharray="3 3" />
          <line x1={paddingX} y1={height - paddingY} x2={width - paddingX} y2={height - paddingY} stroke="currentColor" strokeOpacity={0.08} strokeDasharray="3 3" />

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
                <line x1={x} y1={yHigh} x2={x} y2={yLow} stroke={color} strokeWidth={1.5} />
                <rect
                  x={x - candleWidth / 2}
                  y={topBody}
                  width={candleWidth}
                  height={bodyHeight}
                  fill={color}
                  rx={1}
                />
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
      <CardHeader className="flex flex-col sm:flex-row sm:items-center justify-between pb-3 gap-2">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <LineChart className="h-5 w-5 text-emerald-500" />
            <span>マルチタイムフレーム (MTF) チャートプレビュー</span>
          </CardTitle>
          <CardDescription className="text-xs">
            Rust (plotters) が推論時に自動生成する 1600x1200 4分割PNG & PA特徴量
          </CardDescription>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {/* 表示モード切替 */}
          <div className="flex items-center rounded-lg border bg-muted/50 p-0.5">
            <button
              type="button"
              onClick={() => setChartMode("png")}
              className={`flex items-center gap-1 px-2.5 py-1 text-xs font-medium rounded-md transition-colors ${
                chartMode === "png"
                  ? "bg-background text-foreground shadow-xs"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <ImageIcon className="h-3.5 w-3.5" />
              Rust実PNG
            </button>
            <button
              type="button"
              onClick={() => setChartMode("svg")}
              className={`flex items-center gap-1 px-2.5 py-1 text-xs font-medium rounded-md transition-colors ${
                chartMode === "svg"
                  ? "bg-background text-foreground shadow-xs"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <LayoutGrid className="h-3.5 w-3.5" />
              SVG軽量表示
            </button>
          </div>

          {/* 再生成ボタン */}
          <Button
            variant="outline"
            size="sm"
            onClick={handleRegenerate}
            disabled={isRegenerating}
            className="h-8 gap-1.5 text-xs cursor-pointer"
          >
            {isRegenerating ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5" />
            )}
            チャート更新
          </Button>
        </div>
      </CardHeader>

      <CardContent>
        {chartMode === "png" ? (
          <div className="rounded-lg border bg-neutral-950 overflow-hidden relative min-h-[320px] flex items-center justify-center">
            {imageError ? (
              <div className="text-center p-8 space-y-3">
                <ImageIcon className="h-10 w-10 text-muted-foreground mx-auto" />
                <p className="text-sm text-muted-foreground">
                  チャート画像がまだ生成されていません。
                </p>
                <Button
                  size="sm"
                  onClick={handleRegenerate}
                  className="gap-2 bg-emerald-600 hover:bg-emerald-700"
                >
                  <Sparkles className="h-4 w-4" />
                  今すぐRustでチャートを生成
                </Button>
              </div>
            ) : (
              <img
                src={chartUrl}
                alt="Rust Generated Multi-Timeframe Chart"
                className="w-full h-auto object-contain rounded-md"
                onError={() => setImageError(true)}
              />
            )}
          </div>
        ) : (
          /* SVG 4分割グリッド表示 */
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
        )}
      </CardContent>
    </Card>
  );
}
