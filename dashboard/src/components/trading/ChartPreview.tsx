"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { LineChart, RefreshCw, Image as ImageIcon, LayoutGrid, Loader2 } from "lucide-react";
import { tradingApi, CHART_TIMEFRAMES, type ChartTimeframe } from "@/lib/trading-api";

const NO_ERRORS = Object.fromEntries(CHART_TIMEFRAMES.map((tf) => [tf, false])) as Record<ChartTimeframe, boolean>;

/** 時間足別のチャート。画像はエンジンが cTrader の足から要求のたびに描く */
export function ChartPreview() {
  const [mode, setMode] = React.useState<"tabs" | "grid">("tabs");
  // 0 の間は対象ペアの取得待ち。変えると画像を描き直させる
  const [cacheKey, setCacheKey] = React.useState<number>(0);
  const [errored, setErrored] = React.useState<Record<ChartTimeframe, boolean>>(NO_ERRORS);
  /** 設定の対象ペア。先頭が既定 */
  const [pairs, setPairs] = React.useState<string[]>([]);
  const [pair, setPair] = React.useState<string>("");

  React.useEffect(() => {
    let cancelled = false;
    tradingApi
      .getSettings()
      .then((res) => {
        if (cancelled || !res.success || !res.data) return;
        setPairs(res.data.pairs);
        setPair(res.data.pairs[0] ?? "");
        setCacheKey(Date.now());
      })
      .catch(() => {
        // エンジン未起動時も既定ペアで画像を試す（失敗は画像側の onError で表示される）
        if (!cancelled) setCacheKey(Date.now());
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const reload = () => {
    setErrored(NO_ERRORS);
    setCacheKey(Date.now());
  };

  const selectPair = (next: string) => {
    setPair(next);
    reload();
  };

  const renderImage = (tf: ChartTimeframe) => (
    <div className="rounded-lg border bg-muted/30 overflow-hidden relative min-h-[240px] flex items-center justify-center">
      {errored[tf] ? (
        <div className="text-center p-6 space-y-2">
          <ImageIcon className="h-8 w-8 text-muted-foreground mx-auto" />
          <p className="text-sm text-muted-foreground">{tf} のチャートを描けませんでした。cTrader に接続しているか確認してください。</p>
        </div>
      ) : cacheKey === 0 ? (
        <div className="text-center p-6 space-y-2">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground mx-auto" />
        </div>
      ) : (
        // eslint-disable-next-line @next/next/no-img-element
        <img
          src={tradingApi.getLatestChartUrl(tf, cacheKey, pair || undefined)}
          alt={`${pair} ${tf} chart`}
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
            <span>チャート</span>
          </CardTitle>
          <CardDescription className="text-xs">
            cTrader の直近の足と、保有中の建玉・予約注文の水準（建値・SL・TP）。開くたび・再読込のたびに描き直します
          </CardDescription>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {pairs.length > 1 && (
            <Select value={pair} items={Object.fromEntries(pairs.map((p) => [p, p]))} onValueChange={(v) => selectPair(String(v ?? ""))}>
              <SelectTrigger size="sm" className="h-8 w-28 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {pairs.map((p) => (
                  <SelectItem key={p} value={p}>
                    {p}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
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
              一覧
            </Button>
          </div>

          <Button variant="outline" size="sm" onClick={reload} disabled={cacheKey === 0} className="h-8 gap-1.5 text-xs">
            <RefreshCw className="h-3.5 w-3.5" />
            再読込
          </Button>
        </div>
      </CardHeader>

      <CardContent className="space-y-3">
        {mode === "tabs" ? (
          <Tabs defaultValue="1H">
            <TabsList>
              {CHART_TIMEFRAMES.map((tf) => (
                <TabsTrigger key={tf} value={tf} className="text-xs">
                  {tf}
                </TabsTrigger>
              ))}
            </TabsList>
            {CHART_TIMEFRAMES.map((tf) => (
              <TabsContent key={tf} value={tf}>
                {renderImage(tf)}
              </TabsContent>
            ))}
          </Tabs>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {CHART_TIMEFRAMES.map((tf) => (
              <div key={tf} className="space-y-1.5">
                <p className="text-xs font-medium">{tf}</p>
                {renderImage(tf)}
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
