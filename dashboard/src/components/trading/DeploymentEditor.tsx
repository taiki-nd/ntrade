"use client";

import * as React from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Pencil, Plus, Trash2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { tradingApi } from "@/lib/trading-api";
import { defaultParams, formatParams, TIMEFRAME_LABELS } from "@/lib/strategy";
import {
  STRATEGY_TIMEFRAMES,
  type Deployment,
  type StrategyInfo,
  type StrategyParamInfo,
  type StrategyTimeframe,
} from "@/types/trading";

interface DeploymentEditorProps {
  deployments: Deployment[];
  /** 取引設定の対象銘柄（デプロイの銘柄はこの中から選ぶ） */
  pairs: string[];
  onChange: (deployments: Deployment[]) => void;
}

/** 編集ダイアログの入力中の状態（パラメータは入力のまま文字列で持つ） */
interface DraftDeployment {
  id: string;
  pair: string;
  name: string;
  timeframe: StrategyTimeframe;
  params: Record<string, string>;
  enabled: boolean;
  dryRun: boolean;
}

const ID_PATTERN = /^[A-Za-z0-9_-]{1,48}$/;
const TF_ITEMS = Object.fromEntries(STRATEGY_TIMEFRAMES.map((tf) => [tf, TIMEFRAME_LABELS[tf]]));

const toDraftParams = (params: Record<string, number>) =>
  Object.fromEntries(Object.entries(params).map(([k, v]) => [k, String(v)]));

/** 戦略の定義が無い（カタログに無い名前の）ときは、設定済みのパラメータをそのまま入力欄にする */
function paramSpecs(info: StrategyInfo | undefined, draft: DraftDeployment): StrategyParamInfo[] {
  if (info) return info.params;
  return Object.keys(draft.params).map((name) => ({ name, default: 0, min: -Infinity, max: Infinity, integer: false, description: "" }));
}

/** 取引設定の deployments の編集（保存は親フォームの「保存」でまとめて PUT する） */
export function DeploymentEditor({ deployments, pairs, onChange }: DeploymentEditorProps) {
  const [catalog, setCatalog] = React.useState<StrategyInfo[]>([]);
  // 編集中のデプロイ。index が null なら新規
  const [editing, setEditing] = React.useState<{ index: number | null; draft: DraftDeployment } | null>(null);

  React.useEffect(() => {
    tradingApi
      .getStrategies()
      .then((res) => res.success && res.data && setCatalog(res.data))
      .catch(() => undefined);
  }, []);

  const openNew = () => {
    const info = catalog[0];
    const pair = pairs[0] ?? "";
    const base = `${info?.name ?? "strategy"}-${pair.replace(/[^A-Za-z0-9_-]/g, "")}`.slice(0, 44);
    // 既存の ID と被らないよう連番を付ける
    let id = base;
    for (let n = 2; deployments.some((d) => d.id === id); n++) id = `${base}-${n}`;
    setEditing({
      index: null,
      draft: {
        id,
        pair,
        name: info?.name ?? "",
        timeframe: "M5",
        params: toDraftParams(defaultParams(info)),
        // 新しいデプロイはまず止めた状態・ドライランで置く
        enabled: false,
        dryRun: true,
      },
    });
  };

  const openEdit = (index: number) => {
    const d = deployments[index];
    const info = catalog.find((s) => s.name === d.strategy.name);
    setEditing({
      index,
      draft: {
        id: d.id,
        pair: d.pair,
        name: d.strategy.name,
        timeframe: d.strategy.timeframe,
        params: toDraftParams({ ...defaultParams(info), ...d.strategy.params }),
        enabled: d.enabled,
        dryRun: d.dryRun,
      },
    });
  };

  const update = (index: number, patch: Partial<Deployment>) =>
    onChange(deployments.map((d, i) => (i === index ? { ...d, ...patch } : d)));

  const draft = editing?.draft;
  const info = draft ? catalog.find((s) => s.name === draft.name) : undefined;
  const setDraft = (patch: Partial<DraftDeployment>) => editing && setEditing({ ...editing, draft: { ...editing.draft, ...patch } });

  // 入力の検証（エンジン側でも同じ検証をして、通らなければ保存時にエラーを返す）
  const errors: string[] = [];
  if (draft) {
    if (!ID_PATTERN.test(draft.id)) errors.push("ID は英数字と - _ の 1〜48 文字にしてください");
    if (deployments.some((d, i) => d.id === draft.id && i !== editing?.index)) errors.push("同じ ID のデプロイがあります");
    if (!pairs.includes(draft.pair)) errors.push("銘柄は対象銘柄の中から選んでください");
    if (!draft.name) errors.push("戦略を選んでください");
    for (const p of paramSpecs(info, draft)) {
      const raw = draft.params[p.name] ?? "";
      if (raw.trim() === "") continue;
      const v = Number(raw);
      if (!Number.isFinite(v) || v < p.min || v > p.max) errors.push(`${p.name} は ${p.min}〜${p.max} の範囲で入力してください`);
      else if (p.integer && !Number.isInteger(v)) errors.push(`${p.name} は整数で入力してください`);
    }
  }

  const apply = () => {
    if (!editing || !draft || errors.length > 0) return;
    const next: Deployment = {
      id: draft.id,
      pair: draft.pair,
      strategy: {
        name: draft.name,
        timeframe: draft.timeframe,
        // 空欄のパラメータは省略して、戦略の既定値に任せる
        params: Object.fromEntries(
          Object.entries(draft.params).flatMap(([k, v]) => (v.trim() === "" ? [] : [[k, Number(v)]]))
        ),
      },
      enabled: draft.enabled,
      dryRun: draft.dryRun,
    };
    onChange(editing.index === null ? [...deployments, next] : deployments.map((d, i) => (i === editing.index ? next : d)));
    setEditing(null);
  };

  return (
    <div className="space-y-3">
      <div className="overflow-x-auto rounded-md border">
        <Table>
          <TableHeader className="bg-muted/40">
            <TableRow>
              <TableHead className="text-xs">ID</TableHead>
              <TableHead className="text-xs">銘柄</TableHead>
              <TableHead className="text-xs">戦略</TableHead>
              <TableHead className="text-xs text-center">有効</TableHead>
              <TableHead className="text-xs text-center">ドライラン</TableHead>
              <TableHead className="w-[76px]" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {deployments.length === 0 ? (
              <TableRow>
                <TableCell colSpan={6} className="h-16 text-center text-xs text-muted-foreground">
                  デプロイがありません。「デプロイを追加」から戦略を銘柄に割り当ててください
                </TableCell>
              </TableRow>
            ) : (
              deployments.map((d, i) => (
                <TableRow key={`${d.id}-${i}`}>
                  <TableCell className="text-xs font-mono font-semibold">{d.id}</TableCell>
                  <TableCell className={cn("text-xs font-mono", !pairs.includes(d.pair) && "text-loss")} title={!pairs.includes(d.pair) ? "対象銘柄にありません" : undefined}>
                    {d.pair}
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center gap-1.5">
                      <span className="text-xs font-mono">{d.strategy.name}</span>
                      <Badge variant="outline" className="font-mono text-[10px]">
                        {TIMEFRAME_LABELS[d.strategy.timeframe] ?? d.strategy.timeframe}
                      </Badge>
                    </div>
                    <div className="text-[11px] font-mono text-muted-foreground">{formatParams(d.strategy.params) || "既定パラメータ"}</div>
                  </TableCell>
                  <TableCell className="text-center">
                    <Switch checked={d.enabled} onCheckedChange={(v) => update(i, { enabled: v })} aria-label={`${d.id} を有効にする`} />
                  </TableCell>
                  <TableCell className="text-center">
                    <Switch checked={d.dryRun} onCheckedChange={(v) => update(i, { dryRun: v })} aria-label={`${d.id} をドライランにする`} />
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center justify-end gap-1">
                      <Button variant="ghost" size="icon-sm" onClick={() => openEdit(i)} aria-label={`${d.id} を編集`} title="編集">
                        <Pencil className="h-3.5 w-3.5" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        onClick={() => onChange(deployments.filter((_, j) => j !== i))}
                        className="text-muted-foreground hover:text-destructive"
                        aria-label={`${d.id} を削除`}
                        title="削除"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>
      <Button variant="outline" size="sm" onClick={openNew} disabled={pairs.length === 0} className="h-8 gap-1 text-xs">
        <Plus className="h-3.5 w-3.5" />
        デプロイを追加
      </Button>

      <Dialog open={!!editing} onOpenChange={(open) => !open && setEditing(null)}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle className="text-base">{editing?.index === null ? "デプロイを追加" : "デプロイを編集"}</DialogTitle>
            <DialogDescription className="text-xs">
              ここでの変更は、取引設定の「保存」を押すまで反映されません
            </DialogDescription>
          </DialogHeader>
          {draft && (
            <div className="space-y-3">
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">ID（英数字と - _）</Label>
                  <Input value={draft.id} onChange={(e) => setDraft({ id: e.target.value })} className="h-8 text-xs font-mono" />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">銘柄</Label>
                  <Select
                    value={draft.pair}
                    items={Object.fromEntries(pairs.map((p) => [p, p]))}
                    onValueChange={(v) => setDraft({ pair: String(v ?? "") })}
                  >
                    <SelectTrigger size="sm" className="h-8 w-full text-xs">
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
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">戦略</Label>
                  <Select
                    value={draft.name}
                    items={Object.fromEntries(catalog.map((s) => [s.name, s.title]))}
                    onValueChange={(v) => {
                      const name = String(v ?? "");
                      setDraft({ name, params: toDraftParams(defaultParams(catalog.find((s) => s.name === name))) });
                    }}
                  >
                    <SelectTrigger size="sm" className="h-8 w-full text-xs">
                      <SelectValue placeholder="読み込み中..." />
                    </SelectTrigger>
                    <SelectContent>
                      {catalog.map((s) => (
                        <SelectItem key={s.name} value={s.name}>
                          {s.title}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">時間足（この足の確定ごとに動く）</Label>
                  <Select value={draft.timeframe} items={TF_ITEMS} onValueChange={(v) => setDraft({ timeframe: v as StrategyTimeframe })}>
                    <SelectTrigger size="sm" className="h-8 w-full text-xs">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {STRATEGY_TIMEFRAMES.map((tf) => (
                        <SelectItem key={tf} value={tf}>
                          {TIMEFRAME_LABELS[tf]}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>

              {info && <p className="text-xs text-muted-foreground">{info.description}</p>}
              {paramSpecs(info, draft).length > 0 && (
                <div className="grid grid-cols-2 gap-3 rounded-lg border bg-muted/30 p-3">
                  {paramSpecs(info, draft).map((p) => (
                    <div key={p.name} className="space-y-1">
                      <Label className="text-xs font-mono" title={p.description}>
                        {p.name}
                      </Label>
                      <Input
                        type="number"
                        step={p.integer ? "1" : "any"}
                        min={Number.isFinite(p.min) ? p.min : undefined}
                        max={Number.isFinite(p.max) ? p.max : undefined}
                        value={draft.params[p.name] ?? ""}
                        placeholder={info ? String(p.default) : undefined}
                        onChange={(e) => setDraft({ params: { ...draft.params, [p.name]: e.target.value } })}
                        className="h-8 text-xs font-mono"
                      />
                      {info && (
                        <p className="text-[11px] text-muted-foreground">
                          {p.min}〜{p.max}（既定 {p.default}）{p.description && ` ${p.description}`}
                        </p>
                      )}
                    </div>
                  ))}
                </div>
              )}

              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <Switch checked={draft.enabled} onCheckedChange={(v) => setDraft({ enabled: v })} />
                  <Label className="text-xs">有効（自動売買の稼働中、足の確定ごとに戦略を動かす）</Label>
                </div>
                <div className="flex items-center gap-2">
                  <Switch checked={draft.dryRun} onCheckedChange={(v) => setDraft({ dryRun: v })} />
                  <Label className="text-xs">ドライラン: 発注せず記録のみ（シグナル画面で確かめる）</Label>
                </div>
              </div>

              {errors.length > 0 && (
                <ul className="space-y-0.5 text-[11px] text-loss">
                  {errors.map((e) => (
                    <li key={e}>{e}</li>
                  ))}
                </ul>
              )}
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" size="sm" onClick={() => setEditing(null)}>
              キャンセル
            </Button>
            <Button size="sm" onClick={apply} disabled={errors.length > 0}>
              {editing?.index === null ? "追加" : "変更を反映"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
