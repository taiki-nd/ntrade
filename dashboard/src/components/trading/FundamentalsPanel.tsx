"use client";

import * as React from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Landmark, Loader2, RefreshCw } from "lucide-react";
import { tradingApi, type FundamentalFacts, type FundamentalObservation } from "@/lib/trading-api";

const num = (v: number | null | undefined, digits = 2) =>
  v == null ? "—" : v.toLocaleString("ja-JP", { maximumFractionDigits: digits });

const obs = (o: FundamentalObservation | null) => (o ? `${num(o.value, 3)}（${o.date}）` : "—");

/** 判断に渡しているファンダメンタルズの事実（LLM に渡すものと同じ JSON）をそのまま見せる */
export function FundamentalsPanel() {
  const [pairs, setPairs] = React.useState<string[]>([]);
  const [pair, setPair] = React.useState("");
  const [facts, setFacts] = React.useState<FundamentalFacts | null>(null);
  const [loading, setLoading] = React.useState(false);

  const load = React.useCallback(async (target: string) => {
    setLoading(true);
    try {
      const res = await tradingApi.getFundamentals(target || undefined);
      setFacts(res.success && res.data ? res.data : null);
    } catch {
      setFacts(null);
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    let cancelled = false;
    tradingApi
      .getSettings()
      .then((res) => {
        if (cancelled || !res.success || !res.data) return;
        const first = res.data.pairs[0] ?? "";
        setPairs(res.data.pairs);
        setPair(first);
        load(first);
      })
      .catch(() => {
        // エンジン未起動時はページ側のアラートで表示する
      });
    return () => {
      cancelled = true;
    };
  }, [load]);

  const selectPair = (next: string) => {
    setPair(next);
    load(next);
  };

  return (
    <Card className="shadow-xs">
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1.5">
            <CardTitle className="text-base font-semibold flex items-center gap-2">
              <Landmark className="h-5 w-5 text-primary" />
              ファンダメンタルズの事実
            </CardTitle>
            <CardDescription className="text-xs">
              公的統計・米日の国債利回り・CFTC 建玉明細・指標予定・公式発表のタイトルを API で取得し、評価を付けずに LLM へ渡しています（LLM は Web を見ません）。
              {facts && ` 判断時点: ${facts.as_of}${facts.profile ? ` / プロファイル: ${facts.profile}` : ""}`}
            </CardDescription>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            {pairs.length > 0 && (
              <Select value={pair} items={Object.fromEntries(pairs.map((p) => [p, p]))} onValueChange={(v) => selectPair(String(v ?? ""))}>
                <SelectTrigger size="sm" className="h-8 w-32 text-xs">
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
            <Button variant="outline" size="sm" onClick={() => load(pair)} disabled={loading} className="h-8 gap-1.5 text-xs">
              {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
              再読込
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-6">
        {!facts ? (
          <p className="text-sm text-muted-foreground">{loading ? "読み込み中..." : "データがありません。"}</p>
        ) : (
          <>
            {facts.unavailable.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {facts.unavailable.map((u) => (
                  <Badge key={u.source} variant="outline" className="font-normal text-muted-foreground" title={u.error ?? undefined}>
                    取得できていない: {u.source}
                    {u.fetched_at ? `（最終 ${u.fetched_at}）` : ""}
                  </Badge>
                ))}
              </div>
            )}

            <section className="space-y-2">
              <h3 className="text-sm font-medium">金利・経済統計</h3>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="text-xs">系列</TableHead>
                    <TableHead className="text-xs">最新</TableHead>
                    <TableHead className="text-xs">前回</TableHead>
                    <TableHead className="text-xs">約1か月前</TableHead>
                    <TableHead className="text-xs">約12か月前</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {facts.series.map((s) => (
                    <TableRow key={s.id}>
                      <TableCell className="text-xs">
                        <div>{s.title}</div>
                        <div className="font-mono text-[11px] text-muted-foreground">
                          {s.id}
                          {s.units ? ` · ${s.units}` : ""}
                        </div>
                      </TableCell>
                      <TableCell className="text-xs font-mono">{obs(s.latest)}</TableCell>
                      <TableCell className="text-xs font-mono">{obs(s.previous)}</TableCell>
                      <TableCell className="text-xs font-mono">{obs(s.about_1m_ago)}</TableCell>
                      <TableCell className="text-xs font-mono">{obs(s.about_12m_ago)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </section>

            {facts.positioning_cftc.length > 0 && (
              <section className="space-y-2">
                <h3 className="text-sm font-medium">CFTC 建玉明細（投機筋）</h3>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead className="text-xs">市場</TableHead>
                      <TableHead className="text-xs">基準日</TableHead>
                      <TableHead className="text-xs">ロング</TableHead>
                      <TableHead className="text-xs">ショート</TableHead>
                      <TableHead className="text-xs">ネット</TableHead>
                      <TableHead className="text-xs">52週ネット 高 / 安</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {facts.positioning_cftc.map((c) => {
                      const w = c.weeks[0];
                      return (
                        <TableRow key={c.code}>
                          <TableCell className="text-xs">
                            <div>{c.market}</div>
                            <div className="text-[11px] text-muted-foreground">{c.trader_group}</div>
                          </TableCell>
                          <TableCell className="text-xs font-mono">{w?.report_date ?? "—"}</TableCell>
                          <TableCell className="text-xs font-mono">{num(w?.long, 0)}</TableCell>
                          <TableCell className="text-xs font-mono">{num(w?.short, 0)}</TableCell>
                          <TableCell className="text-xs font-mono">{num(w?.net, 0)}</TableCell>
                          <TableCell className="text-xs font-mono">
                            {num(c.net_52w_high, 0)} / {num(c.net_52w_low, 0)}
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </section>
            )}

            {facts.economic_calendar.length > 0 && (
              <section className="space-y-2">
                <h3 className="text-sm font-medium">経済指標の予定（UTC）</h3>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead className="text-xs">時刻</TableHead>
                      <TableHead className="text-xs">通貨</TableHead>
                      <TableHead className="text-xs">指標</TableHead>
                      <TableHead className="text-xs">重要度</TableHead>
                      <TableHead className="text-xs">前回</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {facts.economic_calendar.map((e) => (
                      <TableRow key={`${e.time_utc}-${e.currency}-${e.title}`} className={e.released ? "text-muted-foreground" : undefined}>
                        <TableCell className="text-xs font-mono">{e.time_utc}</TableCell>
                        <TableCell className="text-xs">{e.currency}</TableCell>
                        <TableCell className="text-xs">
                          {e.title}
                          {e.released && <span className="ml-1.5 text-[11px]">（予定時刻経過）</span>}
                        </TableCell>
                        <TableCell className="text-xs">{e.impact ?? "—"}</TableCell>
                        <TableCell className="text-xs font-mono">{e.previous ?? "—"}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </section>
            )}

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
              {facts.official_releases.length > 0 && (
                <section className="space-y-2">
                  <h3 className="text-sm font-medium">公式発表（タイトルのみ）</h3>
                  <ul className="space-y-1.5">
                    {facts.official_releases.map((r) => (
                      <li key={`${r.source}-${r.published_utc}-${r.title}`} className="text-xs">
                        <span className="font-mono text-muted-foreground">{r.published_utc}</span>
                        <Badge variant="outline" className="mx-1.5 h-4 px-1 text-[10px] font-normal">
                          {r.source}
                        </Badge>
                        {r.title}
                      </li>
                    ))}
                  </ul>
                </section>
              )}
              {facts.related_markets.length > 0 && (
                <section className="space-y-2">
                  <h3 className="text-sm font-medium">関連銘柄（日足）</h3>
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead className="text-xs">銘柄</TableHead>
                        <TableHead className="text-xs">終値</TableHead>
                        <TableHead className="text-xs">1日</TableHead>
                        <TableHead className="text-xs">5日</TableHead>
                        <TableHead className="text-xs">20日</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {facts.related_markets.map((m) => (
                        <TableRow key={m.symbol}>
                          <TableCell className="text-xs font-mono">{m.symbol}</TableCell>
                          <TableCell className="text-xs font-mono">{num(m.last_daily_close, 5)}</TableCell>
                          <TableCell className="text-xs font-mono">{num(m.change_1d_pct)}%</TableCell>
                          <TableCell className="text-xs font-mono">{num(m.change_5d_pct)}%</TableCell>
                          <TableCell className="text-xs font-mono">{num(m.change_20d_pct)}%</TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </section>
              )}
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
