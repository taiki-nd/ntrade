"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { LineChart, RefreshCw, Image as ImageIcon, LayoutGrid, Loader2 } from "lucide-react";
import {
  tradingApi,
  CHART_TIMEFRAMES,
  type ChartTimeframe,
  type MarketSnapshot,
} from "@/lib/trading-api";
import { toast } from "sonner";

const TF_LABEL: Record<ChartTimeframe, string> = {
  "4H": "4H 環境認識",
  "1H": "1H スイング構造",
  "15M": "15M セットアップ",
  "5M": "5M 攻防",
};

function fmt(v: number | null | undefined, digits = 1): string {
  return v === null || v === undefined ? "-" : v.toFixed(digits);
}

export function ChartPreview() {
  const [mode, setMode] = React.useState<"tabs" | "grid">("tabs");
  const [cacheKey, setCacheKey] = React.useState<number>(() => Date.now());
  const [snapshot, setSnapshot] = React.useState<MarketSnapshot | null>(null);
  const [isRegenerating, setIsRegenerating] = React.useState<boolean>(false);
  const [errored, setErrored] = React.useState<Record<ChartTimeframe, boolean>>({
    "4H": false,
    "1H": false,
    "15M": false,
    "5M": false,
  });

  const loadSnapshot = React.useCallback(async () => {
    try {
      const res = await tradingApi.getLatestSnapshot();
      if (res.success && res.data) setSnapshot(res.data);
    } catch {
      // エンジン未起動時は静かに無視（画像側の onError で表示される）
    }
  }, []);

  React.useEffect(() => {
    let cancelled = false;
    tradingApi
      .getLatestSnapshot()
      .then((res) => {
        if (!cancelled && res.success && res.data) setSnapshot(res.data);
      })
      .catch(() => {
        // エンジン未起動時は静かに無視
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleRegenerate = async () => {
    try {
      setIsRegenerating(true);
      const res = await tradingApi.generateChart();
      if (res.success) {
        setCacheKey(Date.now());
        setErrored({ "4H": false, "1H": false, "15M": false, "5M": false });
        await loadSnapshot();
        toast.success("Snapshot を再生成しました", {
          description: "4H / 1H / 15M / 5M の画像と客観的事実JSONが更新されました。",
        });
      } else {
        toast.error("Snapshot の再生成に失敗しました", { description: res.message });
      }
    } catch (err: unknown) {
      toast.error("Rustエンジンへの接続に失敗しました", {
        description: err instanceof Error ? err.message : "エンジンが起動しているか確認してください。",
      });
    } finally {
      setIsRegenerating(false);
    }
  };

  const renderImage = (tf: ChartTimeframe) => (
    <div className="rounded-lg border bg-muted/30 overflow-hidden relative min-h-[240px] flex items-center justify-center">
      {errored[tf] ? (
        <div className="text-center p-6 space-y-2">
          <ImageIcon className="h-8 w-8 text-muted-foreground mx-auto" />
          <p className="text-sm text-muted-foreground">{tf} の画像がまだありません。</p>
        </div>
      ) : (
        // eslint-disable-next-line @next/next/no-img-element
        <img
          src={tradingApi.getLatestChartUrl(tf, cacheKey)}
          alt={`${tf} chart`}
          className="w-full h-auto object-contain"
          onError={() => setErrored((e) => ({ ...e, [tf]: true }))}
        />
      )}
    </div>
  );

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-col sm:flex-row sm:items-center justify-between pb-3 gap-2">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <LineChart className="h-5 w-5 text-primary" />
            <span>Snapshot: LLM が見ているチャート</span>
          </CardTitle>
          <CardDescription className="text-xs">
            時間足ごとに1枚。ローソク足・EMA20/50・価格ラベル付き水平線のみ（判定は描かない）
          </CardDescription>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <div className="flex items-center rounded-lg border bg-muted/50 p-0.5">
            <Button
              type="button"
              variant={mode === "tabs" ? "secondary" : "ghost"}
              size="sm"
              className="h-7 gap-1 px-2.5 text-xs"
              onClick={() => setMode("tabs")}
            >
              <ImageIcon className="h-3.5 w-3.5" />
              単体
            </Button>
            <Button
              type="button"
              variant={mode === "grid" ? "secondary" : "ghost"}
              size="sm"
              className="h-7 gap-1 px-2.5 text-xs"
              onClick={() => setMode("grid")}
            >
              <LayoutGrid className="h-3.5 w-3.5" />
              4枚
            </Button>
          </div>

          <Button
            variant="outline"
            size="sm"
            onClick={handleRegenerate}
            disabled={isRegenerating}
            className="h-8 gap-1.5 text-xs"
          >
            {isRegenerating ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5" />
            )}
            再生成
          </Button>
        </div>
      </CardHeader>

      <CardContent className="space-y-3">
        {snapshot && (
          <div className="flex flex-wrap items-center gap-1.5 text-xs">
            <Badge variant="outline">{snapshot.pair}</Badge>
            <Badge variant="outline">{snapshot.session}</Badge>
            <Badge variant="outline">{snapshot.timestamp}</Badge>
            <span className="text-muted-foreground">
              ATR14 5M {fmt(snapshot.volatility.atr14_5m_pips)} / 1H {fmt(snapshot.volatility.atr14_1h_pips)} pips
            </span>
            <span className="text-muted-foreground">
              当日レンジ {fmt(snapshot.volatility.today_range_pips)} pips
              {snapshot.volatility.today_range_vs_avg !== null &&
                ` (平均比 ${fmt(snapshot.volatility.today_range_vs_avg, 2)})`}
            </span>
            <span className="text-muted-foreground">
              PDH/PDL {fmt(snapshot.reference_levels.prev_day_high, 3)} / {fmt(snapshot.reference_levels.prev_day_low, 3)}
            </span>
          </div>
        )}

        {mode === "tabs" ? (
          <Tabs defaultValue="5M">
            <TabsList>
              {CHART_TIMEFRAMES.map((tf) => (
                <TabsTrigger key={tf} value={tf} className="text-xs">
                  {tf}
                </TabsTrigger>
              ))}
            </TabsList>
            {CHART_TIMEFRAMES.map((tf) => (
              <TabsContent key={tf} value={tf} className="space-y-1.5">
                <p className="text-xs text-muted-foreground">{TF_LABEL[tf]}</p>
                {renderImage(tf)}
              </TabsContent>
            ))}
          </Tabs>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {CHART_TIMEFRAMES.map((tf) => (
              <div key={tf} className="space-y-1.5">
                <p className="text-xs font-medium">{TF_LABEL[tf]}</p>
                {renderImage(tf)}
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
