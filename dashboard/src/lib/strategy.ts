import type { StrategyInfo, StrategySpec, StrategyTimeframe } from "@/types/trading";

/** 戦略の時間足の表示名（チャート・ヒストリカルの表記に合わせる） */
export const TIMEFRAME_LABELS: Record<StrategyTimeframe, string> = {
  M1: "1M",
  M5: "5M",
  M15: "15M",
  M30: "30M",
  H1: "1H",
  H4: "4H",
  D1: "1D",
};

/** パラメータを "fast=20 slow=50" の形にする */
export function formatParams(params: Record<string, number> | null | undefined): string {
  return Object.entries(params ?? {})
    .map(([k, v]) => `${k}=${v}`)
    .join(" ");
}

/** エンジンの StrategySpec::label と同じ形（"ema_cross 5M fast=20 slow=50"） */
export function strategyLabel(spec: StrategySpec): string {
  return `${spec.name} ${TIMEFRAME_LABELS[spec.timeframe] ?? spec.timeframe} ${formatParams(spec.params)}`.trim();
}

/** 戦略の既定パラメータ */
export function defaultParams(info: StrategyInfo | undefined): Record<string, number> {
  return Object.fromEntries((info?.params ?? []).map((p) => [p.name, p.default]));
}
