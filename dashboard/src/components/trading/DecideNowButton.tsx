"use client";

import * as React from "react";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Loader2, Play } from "lucide-react";
import { toast } from "sonner";
import { tradingApi } from "@/lib/trading-api";

interface DecideNowButtonProps {
  /** 判断サイクル完了後に親の同期を促す */
  onDecided?: () => void;
}

/** 判断間隔を待たずに、選んだ銘柄で判断サイクルを 1 回回す */
export function DecideNowButton({ onDecided }: DecideNowButtonProps) {
  const [deciding, setDeciding] = React.useState(false);
  const [pairs, setPairs] = React.useState<string[]>([]);
  const [pair, setPair] = React.useState<string>("");

  React.useEffect(() => {
    let cancelled = false;
    tradingApi
      .getSettings()
      .then((res) => {
        if (cancelled || !res.success || !res.data) return;
        setPairs(res.data.pairs);
        setPair((cur) => cur || res.data!.pairs[0] || "");
      })
      .catch(() => {
        // エンジン未起動時は無視
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleDecide = async () => {
    setDeciding(true);
    try {
      const res = await tradingApi.decideNow(pair || undefined);
      if (res.success && res.data) {
        const d = res.data;
        const order = d.executed
          ? `予約注文を発注 (#${d.order_id})`
          : d.decision.action === "HOLD"
            ? "新規注文なし"
            : d.guard.passed
              ? "発注できませんでした"
              : `ガードで見送り: ${d.guard.failed.join(", ")}`;
        toast.info(`${pair ? `${pair} ` : ""}${d.decision.action}（確信度 ${(d.decision.confidence * 100).toFixed(0)}%）`, {
          description: [order, ...d.manage_results].join("\n"),
        });
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

  return (
    <div className="flex items-center gap-2">
      {pairs.length > 1 && (
        <Select value={pair} items={Object.fromEntries(pairs.map((p) => [p, p]))} onValueChange={(v) => setPair(String(v ?? ""))}>
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
      <Button size="sm" onClick={handleDecide} disabled={deciding} className="h-8 gap-1.5 text-xs" title="判断間隔を待たずに今すぐ判断させる">
        {deciding ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Play className="h-3.5 w-3.5" />}
        {deciding ? "判断中…" : "今すぐ判断"}
      </Button>
    </div>
  );
}
