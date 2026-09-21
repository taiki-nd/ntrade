"use client";

import * as React from "react";
import { DashboardLayout } from "@/components/layouts/dashboard-layout";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Settings, ShieldCheck, Cpu, RefreshCw, Loader2, TriangleAlert, Link2 } from "lucide-react";
import { toast } from "sonner";
import { tradingApi, type RuntimeInfo, type GuardConfig } from "@/lib/trading-api";
import type { AccountMetrics } from "@/types/trading";

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <TableRow>
      <TableCell className="text-xs text-muted-foreground w-56">{label}</TableCell>
      <TableCell className="text-xs font-mono">{value}</TableCell>
    </TableRow>
  );
}

export default function SettingsPage() {
  const [runtime, setRuntime] = React.useState<RuntimeInfo | null>(null);
  const [guard, setGuard] = React.useState<GuardConfig | null>(null);
  const [status, setStatus] = React.useState<AccountMetrics | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [offline, setOffline] = React.useState(false);

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const [r, g, s] = await Promise.all([tradingApi.getRuntime(), tradingApi.getGuardConfig(), tradingApi.getStatus()]);
      if (r.success && r.data) setRuntime(r.data);
      if (g.success && g.data) setGuard(g.data);
      setStatus(s);
      setOffline(false);
    } catch {
      setOffline(true);
    } finally {
      setLoading(false);
    }
  }, []);

  const [linking, setLinking] = React.useState(false);

  // Spotware の OAuth 認可画面へ遷移。承認後 /auth/ctrader/callback でトークンが SQLite に保存される
  const startOAuth = async () => {
    setLinking(true);
    try {
      const res = await tradingApi.getOAuthUrl();
      if (res.success && res.data?.url) {
        window.location.href = res.data.url;
        return;
      }
      toast.error("OAuth URL の取得に失敗しました", { description: res.message || ".env の CTRADER_CLIENT_ID を確認してください。" });
    } catch {
      toast.error("Rustコアエンジン (localhost:4000) に接続できません");
    }
    setLinking(false);
  };

  React.useEffect(() => {
    const id = setTimeout(load, 0);
    return () => clearTimeout(id);
  }, [load]);

  return (
    <DashboardLayout title="システム設定">
      <div className="space-y-6 max-w-4xl">
        <div className="flex items-center justify-between">
          <p className="text-sm text-muted-foreground">
            設定は環境変数（<code className="font-mono text-xs">.env</code>）と <code className="font-mono text-xs">config/guard.toml</code> で行い、ここでは現在値を確認します。変更後はエンジンの再起動が必要です。
          </p>
          <Button variant="outline" size="sm" onClick={load} disabled={loading} className="h-8 gap-1.5 text-xs">
            {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
            再読込
          </Button>
        </div>

        {offline && (
          <Alert variant="destructive">
            <TriangleAlert className="h-4 w-4" />
            <AlertTitle>Rustコアエンジンに接続できません</AlertTitle>
            <AlertDescription>エンジン（localhost:4000）を起動してから再読込してください。</AlertDescription>
          </Alert>
        )}

        {/* 稼働設定 */}
        <Card className="shadow-xs">
          <CardHeader className="pb-3">
            <CardTitle className="text-base font-semibold flex items-center gap-2">
              <Cpu className="h-5 w-5 text-primary" />
              稼働設定
            </CardTitle>
            <CardDescription className="text-xs">スケジューラ、発注モード、LLM CLI の設定（環境変数）</CardDescription>
          </CardHeader>
          <CardContent>
            {runtime ? (
              <Table>
                <TableBody>
                  <Row
                    label="発注モード (NTRADE_LIVE_ORDERS)"
                    value={
                      <Badge variant={runtime.orderMode === "live" ? "destructive" : "secondary"}>
                        {runtime.orderMode === "live" ? "LIVE: cTrader に実発注" : "PAPER: 発注しない"}
                      </Badge>
                    }
                  />
                  <Row label="スケジューラ (NTRADE_SCHEDULER)" value={runtime.schedulerEnabled ? "有効（5分足確定ごと）" : "無効"} />
                  <Row label="対象ペア (NTRADE_PAIRS)" value={runtime.pairs.join(", ")} />
                  <Row label="足確定後の待ち秒数 (NTRADE_BAR_DELAY_SECS)" value={`${runtime.barDelaySecs} 秒`} />
                  <Row label="ペーパー初期残高 (NTRADE_PAPER_BALANCE)" value={runtime.paperBalance.toLocaleString()} />
                  <Row label="LLM CLI" value={`${runtime.llmCli} (timeout ${runtime.llmTimeoutSecs}s)`} />
                  <Row label="ガード設定ファイル" value={runtime.guardConfigPath} />
                  <Row label="SQLite" value={runtime.dbPath} />
                  <Row label=".env" value={runtime.envFilePresent ? "あり" : "なし（.env.example をコピーしてください）"} />
                </TableBody>
              </Table>
            ) : (
              <p className="text-sm text-muted-foreground">読み込み中...</p>
            )}
          </CardContent>
        </Card>

        {/* 接続状態 */}
        <Card className="shadow-xs">
          <CardHeader className="pb-3">
            <div className="flex items-start justify-between gap-4">
              <div className="space-y-1.5">
                <CardTitle className="text-base font-semibold flex items-center gap-2">
                  <Settings className="h-5 w-5 text-primary" />
                  接続と口座
                </CardTitle>
                <CardDescription className="text-xs">
                  cTrader と連携すると Access Token / Refresh Token が SQLite に保存され、期限の3日前に自動更新されます。
                </CardDescription>
              </div>
              <Button size="sm" onClick={startOAuth} disabled={linking} className="h-8 gap-1.5 text-xs shrink-0">
                {linking ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Link2 className="h-3.5 w-3.5" />}
                {status?.connectionStatus.ctrader === "connected" ? "cTrader と再連携" : "cTrader と連携"}
              </Button>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {runtime && runtime.ctraderTokenPresent && !runtime.ctraderRefreshTokenPresent && (
              <Alert>
                <TriangleAlert className="h-4 w-4" />
                <AlertTitle>トークンの自動更新が無効です</AlertTitle>
                <AlertDescription>
                  Refresh Token がないため、Access Token の期限が切れると接続できなくなります。「cTrader を再連携」から認可してください。
                </AlertDescription>
              </Alert>
            )}
            {status ? (
              <Table>
                <TableBody>
                  <Row label="cTrader" value={`${status.connectionStatus.ctrader} / ${status.connectionStatus.environment} / ${status.connectionStatus.accountNumber}`} />
                  {runtime && (
                    <>
                      <Row
                        label="トークン自動更新"
                        value={
                          <Badge variant={runtime.ctraderRefreshTokenPresent ? "secondary" : "destructive"}>
                            {runtime.ctraderRefreshTokenPresent ? "有効" : runtime.ctraderTokenPresent ? "無効（Refresh Token なし）" : "未連携"}
                          </Badge>
                        }
                      />
                      <Row label="Access Token 期限 (UTC)" value={runtime.ctraderTokenExpiresAt ?? "不明（次回の更新で取得）"} />
                    </>
                  )}
                  <Row label="ブローカー口座残高" value={status.brokerBalance !== undefined ? status.brokerBalance.toLocaleString() : "未取得"} />
                  <Row label="表示中の残高（発注モード基準）" value={`${status.balance.toLocaleString()} (${status.orderMode})`} />
                  <Row label="ボット状態" value={status.botState} />
                </TableBody>
              </Table>
            ) : (
              <p className="text-sm text-muted-foreground">読み込み中...</p>
            )}
          </CardContent>
        </Card>

        {/* ガード設定 */}
        <Card className="shadow-xs">
          <CardHeader className="pb-3">
            <CardTitle className="text-base font-semibold flex items-center gap-2">
              <ShieldCheck className="h-5 w-5 text-primary" />
              事後ガード (config/guard.toml)
            </CardTitle>
            <CardDescription className="text-xs">LLM の判断を発注前に機械的に検証する閾値。判断ロジックではなくリスク管理。</CardDescription>
          </CardHeader>
          <CardContent>
            {guard ? (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="text-xs">項目</TableHead>
                    <TableHead className="text-xs">値</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <Row label="観測整合チェック" value={guard.observed_check ? "有効" : "無効"} />
                  <Row label="確信度の下限" value={guard.min_confidence.toFixed(2)} />
                  <Row label="リスクリワード下限" value={guard.min_rr.toFixed(2)} />
                  <Row label="SL 幅 (5M ATR 比)" value={`${guard.sl_atr_min} 〜 ${guard.sl_atr_max}`} />
                  <Row label="SL 最小幅" value={`${guard.sl_min_pips} pips`} />
                  <Row label="ポジション上限" value={`ペアごと ${guard.max_positions_per_pair} / 全体 ${guard.max_positions_total}`} />
                  <Row label="日次損失上限" value={`${guard.daily_loss_limit_pct}%`} />
                  <Row label="条件付きプラン最長" value={`${guard.plan_max_hours} 時間`} />
                  <Row label="ロット" value={guard.risk_pct ? `リスク ${guard.risk_pct}% から算出` : `固定 ${guard.fixed_volume_lots} lot`} />
                  <Row
                    label="スプレッド上限"
                    value={Object.entries(guard.max_spread_pips)
                      .map(([k, v]) => `${k}: ${v}`)
                      .join(", ")}
                  />
                  <Row label="指標ブラックアウト" value={guard.news_blackout.length === 0 ? "なし" : guard.news_blackout.map((b) => `${b.label || ""} ${b.time} (-${b.before_min}/+${b.after_min}分)`).join(" / ")} />
                </TableBody>
              </Table>
            ) : (
              <p className="text-sm text-muted-foreground">読み込み中...</p>
            )}
          </CardContent>
        </Card>
      </div>
    </DashboardLayout>
  );
}
