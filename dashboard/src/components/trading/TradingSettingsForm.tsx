"use client";

import * as React from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { ShieldCheck, Loader2, Plus, Save, Undo2, X } from "lucide-react";
import { toast } from "sonner";
import { tradingApi, type GuardConfig, type TradingSettings } from "@/lib/trading-api";

type NumericGuardKey = {
  [K in keyof GuardConfig]: GuardConfig[K] extends number ? K : never;
}[keyof GuardConfig];

type NullableGuardKey = "risk_pct" | "pip_value_per_lot";

interface NumberFieldProps {
  label: string;
  hint?: string;
  value: number | null;
  step?: string;
  placeholder?: string;
  onChange: (v: number | null) => void;
}

/** 空欄は null として返す（必須項目は呼び出し側で 0 に倒し、エンジン側の検証で弾く） */
function NumberField({ label, hint, value, step = "any", placeholder, onChange }: NumberFieldProps) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Input
        type="number"
        step={step}
        value={value ?? ""}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value === "" ? null : Number(e.target.value))}
        className="h-8 text-xs font-mono"
      />
      {hint && <p className="text-[11px] text-muted-foreground">{hint}</p>}
    </div>
  );
}

export function TradingSettingsForm() {
  const [saved, setSaved] = React.useState<TradingSettings | null>(null);
  const [draft, setDraft] = React.useState<TradingSettings | null>(null);
  const [newPair, setNewPair] = React.useState("");
  const [saving, setSaving] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      const res = await tradingApi.getSettings();
      if (res.success && res.data) {
        setSaved(res.data);
        setDraft(res.data);
      }
    } catch {
      // 接続エラーはページ側のアラートで表示する
    }
  }, []);

  React.useEffect(() => {
    const id = setTimeout(load, 0);
    return () => clearTimeout(id);
  }, [load]);

  if (!draft) {
    return (
      <Card className="shadow-xs">
        <CardContent className="py-6">
          <p className="text-sm text-muted-foreground">読み込み中...</p>
        </CardContent>
      </Card>
    );
  }

  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const g = draft.guard;
  const setGuard = (patch: Partial<GuardConfig>) => setDraft({ ...draft, guard: { ...g, ...patch } });
  const setNum = (key: NumericGuardKey) => (v: number | null) => setGuard({ [key]: v ?? 0 } as Partial<GuardConfig>);
  const setNullable = (key: NullableGuardKey) => (v: number | null) => setGuard({ [key]: v } as Partial<GuardConfig>);

  const addPair = () => {
    // ブローカーの銘柄名をそのまま使う（ゼロ口座は USDJPY_z）。表記の正規化は保存時にエンジンが一覧に合わせて行う
    const p = newPair.trim().replace("/", "");
    if (!p) return;
    if (!draft.pairs.some((x) => x.toLowerCase() === p.toLowerCase())) setDraft({ ...draft, pairs: [...draft.pairs, p] });
    setNewPair("");
  };
  const removePair = (p: string) => setDraft({ ...draft, pairs: draft.pairs.filter((x) => x !== p) });
  const makeDefault = (p: string) => setDraft({ ...draft, pairs: [p, ...draft.pairs.filter((x) => x !== p)] });

  const spreads = Object.entries(g.max_spread_pips);
  const setSpreads = (rows: [string, number][]) => setGuard({ max_spread_pips: Object.fromEntries(rows) });

  const setBlackout = (i: number, patch: Partial<GuardConfig["news_blackout"][number]>) =>
    setGuard({ news_blackout: g.news_blackout.map((b, j) => (j === i ? { ...b, ...patch } : b)) });

  const save = async () => {
    setSaving(true);
    try {
      const res = await tradingApi.saveSettings(draft);
      if (res.success && res.data) {
        setSaved(res.data);
        setDraft(res.data);
        toast.success("設定を保存しました", { description: "次の判断サイクルから反映されます。" });
      } else {
        toast.error("保存できませんでした", { description: res.message });
      }
    } catch {
      toast.error("Rustコアエンジン (localhost:4000) に接続できません");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card className="shadow-xs">
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1.5">
            <CardTitle className="text-base font-semibold flex items-center gap-2">
              <ShieldCheck className="h-5 w-5 text-primary" />
              取引設定
            </CardTitle>
            <CardDescription className="text-xs">
              SQLite に保存され、再起動なしで次の判断サイクルから反映されます。ガードは判断ロジックではなくリスク管理です。
            </CardDescription>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <Button variant="outline" size="sm" onClick={() => setDraft(saved)} disabled={!dirty || saving} className="h-8 gap-1.5 text-xs">
              <Undo2 className="h-3.5 w-3.5" />
              元に戻す
            </Button>
            <Button size="sm" onClick={save} disabled={!dirty || saving} className="h-8 gap-1.5 text-xs">
              {saving ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Save className="h-3.5 w-3.5" />}
              保存
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* 対象ペア */}
        <section className="space-y-3">
          <div>
            <h3 className="text-sm font-medium">対象ペア</h3>
            <p className="text-xs text-muted-foreground">
              cTrader の銘柄名をそのまま入力します（ゼロ口座なら <code className="font-mono">USDJPY_z</code>。素の <code className="font-mono">USDJPY</code> は発注できません）。
              5分足確定ごとに全ペアを並行に判断し、先頭のペアが画面と API の既定になります（クリックで先頭へ）。
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-1.5">
            {draft.pairs.map((p, i) => (
              <Badge key={p} variant={i === 0 ? "default" : "outline"} className="gap-1 pr-1 font-mono">
                <span className="cursor-pointer" onClick={() => makeDefault(p)} title="既定ペアにする">
                  {p}
                </span>
                <Button variant="ghost" size="icon-xs" onClick={() => removePair(p)} aria-label={`${p} を外す`} className="h-4 w-4">
                  <X className="h-3 w-3" />
                </Button>
              </Badge>
            ))}
          </div>
          <div className="flex items-center gap-2 max-w-xs">
            <Input
              value={newPair}
              placeholder="EURUSD_z"
              onChange={(e) => setNewPair(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addPair()}
              className="h-8 text-xs font-mono"
            />
            <Button variant="outline" size="sm" onClick={addPair} className="h-8 gap-1 text-xs">
              <Plus className="h-3.5 w-3.5" />
              追加
            </Button>
          </div>
          <div className="max-w-xs">
            <NumberField
              label="足確定後の待ち秒数"
              step="1"
              value={draft.barDelaySecs}
              onChange={(v) => setDraft({ ...draft, barDelaySecs: v ?? 0 })}
            />
          </div>
        </section>

        <Separator />

        {/* エントリー検証 */}
        <section className="space-y-3">
          <h3 className="text-sm font-medium">エントリー検証</h3>
          <div className="flex items-center gap-2">
            <Switch checked={g.observed_check} onCheckedChange={(v) => setGuard({ observed_check: v })} />
            <Label className="text-xs">観測整合チェック（LLM が読んだ最新足時刻が Snapshot と一致しなければ HOLD）</Label>
          </div>
          <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
            <NumberField label="確信度の下限 (0〜1)" value={g.min_confidence} onChange={setNum("min_confidence")} />
            <NumberField label="リスクリワード下限" value={g.min_rr} onChange={setNum("min_rr")} />
            <NumberField label="SL 最小幅 (pips)" value={g.sl_min_pips} onChange={setNum("sl_min_pips")} />
            <NumberField label="SL 幅 下限 (5M ATR 比)" value={g.sl_atr_min} onChange={setNum("sl_atr_min")} />
            <NumberField label="SL 幅 上限 (5M ATR 比)" value={g.sl_atr_max} onChange={setNum("sl_atr_max")} />
            <NumberField label="条件付きプラン最長 (時間)" value={g.plan_max_hours} onChange={setNum("plan_max_hours")} />
          </div>
        </section>

        <Separator />

        {/* 損切りの執行 */}
        <section className="space-y-3">
          <h3 className="text-sm font-medium">損切りの執行</h3>
          <div className="flex items-center gap-2">
            <Switch
              checked={g.stop_mode === "close"}
              onCheckedChange={(v) => setGuard({ stop_mode: v ? "close" : "touch" })}
            />
            <Label className="text-xs">
              5M 確定足の終値で損切りを判定する（オフならブローカーの SL にヒゲが触れた時点で決済）
            </Label>
          </div>
          <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
            <NumberField
              label="ハードSLの位置 (5M ATR 比)"
              value={g.hard_stop_atr}
              onChange={setNum("hard_stop_atr")}
              hint="終値判定のとき、LLM の SL からこの幅だけ外側にブローカーの SL を置く"
            />
          </div>
        </section>

        <Separator />

        {/* 資金管理 */}
        <section className="space-y-3">
          <h3 className="text-sm font-medium">ポジションと資金管理</h3>
          <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
            <NumberField label="ポジション上限（ペアごと）" step="1" value={g.max_positions_per_pair} onChange={setNum("max_positions_per_pair")} />
            <NumberField
              label="ポジション上限（全体）"
              step="1"
              value={g.max_positions_total}
              onChange={setNum("max_positions_total")}
              hint={g.max_positions_total < draft.pairs.length ? "ペア数より少ないため、同時に持てないペアがあります" : undefined}
            />
            <NumberField label="日次損失上限 (%)" value={g.daily_loss_limit_pct} onChange={setNum("daily_loss_limit_pct")} />
            <NumberField label="固定ロット" value={g.fixed_volume_lots} onChange={setNum("fixed_volume_lots")} hint="リスク % 未設定時に使用" />
            <NumberField
              label="リスク % (空欄で固定ロット)"
              value={g.risk_pct}
              placeholder="未設定"
              onChange={setNullable("risk_pct")}
              hint="SL 幅と残高からロットを算出"
            />
            <NumberField
              label="1 lot・1 pip の価値 (口座通貨)"
              value={g.pip_value_per_lot}
              placeholder="自動（ブローカーのレート）"
              onChange={setNullable("pip_value_per_lot")}
              hint="空欄ならペアごとにレートから自動計算"
            />
          </div>
        </section>

        <Separator />

        {/* スプレッド上限 */}
        <section className="space-y-3">
          <div>
            <h3 className="text-sm font-medium">スプレッド上限 (pips)</h3>
            <p className="text-xs text-muted-foreground">銘柄名（USDJPY_z など）ごとに上書きできます。一致しない銘柄は default を使います。</p>
          </div>
          <div className="space-y-2 max-w-md">
            {spreads.map(([k, v], i) => (
              <div key={i} className="flex items-center gap-2">
                <Input
                  value={k}
                  disabled={k === "default"}
                  onChange={(e) => setSpreads(spreads.map((r, j) => (j === i ? [e.target.value, r[1]] : r)))}
                  className="h-8 text-xs font-mono"
                />
                <Input
                  type="number"
                  step="any"
                  value={v}
                  onChange={(e) => setSpreads(spreads.map((r, j) => (j === i ? [r[0], Number(e.target.value)] : r)))}
                  className="h-8 w-28 text-xs font-mono"
                />
                <Button
                  variant="ghost"
                  size="icon-sm"
                  disabled={k === "default"}
                  onClick={() => setSpreads(spreads.filter((_, j) => j !== i))}
                  aria-label="削除"
                >
                  <X className="h-3.5 w-3.5" />
                </Button>
              </div>
            ))}
            <Button variant="outline" size="sm" onClick={() => setSpreads([...spreads, ["", 1.0]])} className="h-8 gap-1 text-xs">
              <Plus className="h-3.5 w-3.5" />
              ペアを追加
            </Button>
          </div>
        </section>

        <Separator />

        {/* 指標ブラックアウト */}
        <section className="space-y-3">
          <div>
            <h3 className="text-sm font-medium">指標ブラックアウト</h3>
            <p className="text-xs text-muted-foreground">時刻は UTC（YYYY-MM-DD HH:MM:SS）。前後の分数の間は新規発注しません。</p>
          </div>
          <div className="space-y-2">
            {g.news_blackout.map((b, i) => (
              <div key={i} className="grid grid-cols-[1fr_12rem_5rem_5rem_auto] items-center gap-2">
                <Input value={b.label} placeholder="US CPI" onChange={(e) => setBlackout(i, { label: e.target.value })} className="h-8 text-xs" />
                <Input
                  value={b.time}
                  placeholder="2026-10-15 12:30:00"
                  onChange={(e) => setBlackout(i, { time: e.target.value })}
                  className="h-8 text-xs font-mono"
                />
                <Input
                  type="number"
                  value={b.before_min}
                  title="前（分）"
                  onChange={(e) => setBlackout(i, { before_min: Number(e.target.value) })}
                  className="h-8 text-xs font-mono"
                />
                <Input
                  type="number"
                  value={b.after_min}
                  title="後（分）"
                  onChange={(e) => setBlackout(i, { after_min: Number(e.target.value) })}
                  className="h-8 text-xs font-mono"
                />
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => setGuard({ news_blackout: g.news_blackout.filter((_, j) => j !== i) })}
                  aria-label="削除"
                >
                  <X className="h-3.5 w-3.5" />
                </Button>
              </div>
            ))}
            <Button
              variant="outline"
              size="sm"
              onClick={() => setGuard({ news_blackout: [...g.news_blackout, { label: "", time: "", before_min: 30, after_min: 15 }] })}
              className="h-8 gap-1 text-xs"
            >
              <Plus className="h-3.5 w-3.5" />
              指標を追加
            </Button>
          </div>
        </section>
      </CardContent>
    </Card>
  );
}
