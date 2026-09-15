"use client";

import * as React from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Hourglass, Play, Loader2, Trash2 } from "lucide-react";
import { tradingApi, type PendingPlan } from "@/lib/trading-api";
import { toast } from "sonner";

interface PlanMonitorProps {
  /** 判断サイクル完了後に親の同期を促す */
  onDecided?: () => void;
}

export function PlanMonitor({ onDecided }: PlanMonitorProps) {
  const [plans, setPlans] = React.useState<PendingPlan[]>([]);
  const [deciding, setDeciding] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      const res = await tradingApi.getPlans();
      if (res.success && res.data) setPlans(res.data);
    } catch {
      // エンジン未起動時は無視
    }
  }, []);

  React.useEffect(() => {
    const initial = setTimeout(load, 0);
    const interval = setInterval(load, 10000);
    return () => {
      clearTimeout(initial);
      clearInterval(interval);
    };
  }, [load]);

  const handleDecide = async () => {
    setDeciding(true);
    try {
      const res = await tradingApi.decideNow();
      if (res.success && res.data) {
        const d = res.data;
        const guard = d.guard.passed ? "ガード通過" : `ガード: ${d.guard.failed.join(", ")}`;
        const what = d.executed
          ? "発注（ペーパー）"
          : d.plan_id
            ? "条件付きプランを登録"
            : "見送り";
        toast.info(`判断: ${d.decision.action} (確信度 ${d.decision.confidence.toFixed(2)})`, {
          description: `${what} / ${guard}`,
        });
        await load();
        onDecided?.();
      } else {
        toast.error("判断サイクルに失敗しました", { description: res.message });
      }
    } catch (err: unknown) {
      toast.error("Rustエンジンへの接続に失敗しました", {
        description: err instanceof Error ? err.message : undefined,
      });
    } finally {
      setDeciding(false);
    }
  };

  const handleDiscard = async (id: string) => {
    try {
      await tradingApi.discardPlan(id);
      toast.info("条件付きプランを破棄しました");
      await load();
    } catch {
      toast.error("破棄に失敗しました");
    }
  };

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-col sm:flex-row sm:items-center justify-between pb-3 gap-2">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <Hourglass className="h-5 w-5 text-primary" />
            <span>条件付きプラン</span>
          </CardTitle>
          <CardDescription className="text-xs">
            LLM が「待ち」と判断した条件。5M 確定ごとにプログラムが評価し、成立時はガードを通して執行する。
          </CardDescription>
        </div>
        <Button size="sm" onClick={handleDecide} disabled={deciding} className="h-8 gap-1.5 text-xs">
          {deciding ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Play className="h-3.5 w-3.5" />}
          今すぐ判断
        </Button>
      </CardHeader>
      <CardContent className="space-y-3">
        {plans.length === 0 ? (
          <p className="text-sm text-muted-foreground">保持中のプランはありません。</p>
        ) : (
          plans.map((p) => (
            <div key={p.id} className="rounded-lg border p-3 space-y-2">
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <Badge variant="outline">{p.pair}</Badge>
                  <Badge variant={p.plan.then_action === "BUY" ? "default" : "secondary"}>
                    {p.plan.then_action}
                  </Badge>
                  <span className="text-xs text-muted-foreground">期限 {p.plan.expires_at}</span>
                </div>
                <Button variant="ghost" size="sm" className="h-7 gap-1 text-xs" onClick={() => handleDiscard(p.id)}>
                  <Trash2 className="h-3.5 w-3.5" />
                  破棄
                </Button>
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 text-xs">
                <div>
                  <p className="font-medium">成立条件</p>
                  <p className="text-muted-foreground">{p.plan.wait_for}</p>
                  <p className="font-mono">
                    {p.plan.trigger_condition} {p.plan.trigger_price}
                  </p>
                </div>
                <div>
                  <p className="font-medium">破棄条件</p>
                  <p className="text-muted-foreground">{p.plan.invalidate_if}</p>
                  <p className="font-mono">
                    {p.plan.invalidate_condition} {p.plan.invalidate_price}
                  </p>
                </div>
              </div>
              <p className="text-xs font-mono text-muted-foreground">
                SL {p.plan.stop_loss ?? "-"} / TP {p.plan.take_profit ?? "-"} / 登録 {p.created_at}
              </p>
            </div>
          ))
        )}
      </CardContent>
    </Card>
  );
}
