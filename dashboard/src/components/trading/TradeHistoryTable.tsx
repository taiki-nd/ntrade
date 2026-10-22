"use client";

import { priceDigits } from "@/lib/price";
import * as React from "react";
import { TradeHistory, CloseReason } from "@/types/trading";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  ArrowUpRight,
  ArrowDownRight,
  History,
  CheckCircle,
  AlertTriangle,
  Hand,
  BrainCircuit,
  Bot,
  Radio,
  Search,
  X,
  SlidersHorizontal,
  Trash2,
  ArrowUp,
  ArrowDown,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { isSignalId } from "@/lib/strategy";
import { SignalDetailDialog } from "@/components/trading/SignalViewer";
import { StrategyBadge } from "@/components/trading/PositionTable";
import { PaginationBar, PaginationBarProps } from "@/components/trading/PaginationBar";
import {
  EMPTY_TRADE_FILTER,
  TradeFilterState,
  TradeSortKey,
  countActiveTradeFilters,
} from "@/lib/trading-api";

export interface TradeFilterProps {
  value: TradeFilterState;
  onChange: (value: TradeFilterState) => void;
  /** 銘柄プルダウンの選択肢（未指定なら銘柄の絞り込みは出さない） */
  symbols?: string[];
  /** 戦略（デプロイ ID）プルダウンの選択肢（未指定なら戦略の絞り込みは出さない） */
  strategies?: string[];
}

interface TradeHistoryTableProps {
  trades: TradeHistory[];
  /** 指定時はカード下部にページ送りを表示（ページングはサーバー側） */
  pagination?: PaginationBarProps;
  /** 指定時はヘッダーに検索・絞り込みを表示（絞り込みはサーバー側） */
  filter?: TradeFilterProps;
  /** 指定時は行の削除・一括削除を表示 */
  onDelete?: (ids: string[]) => void | Promise<void>;
}

const SIDE_LABELS: Record<string, string> = { "": "すべて", BUY: "買い (BUY)", SELL: "売り (SELL)" };
const RESULT_LABELS: Record<string, string> = { "": "すべて", win: "勝ちのみ", loss: "負けのみ" };
const CLOSE_REASON_LABELS: Record<string, string> = {
  "": "すべて",
  TAKE_PROFIT: "利確 (TP)",
  STOP_LOSS: "損切り (SL)",
  STRATEGY: "戦略決済",
  MANUAL: "手動決済",
  CIRCUIT_BREAKER: "強制停止",
  INVALIDATED: "旧: 損切り (終値)",
  DISCRETIONARY: "旧: LLM 判断で決済",
};

export function TradeHistoryTable({ trades, pagination, filter, onDelete }: TradeHistoryTableProps) {
  const [selectedSignalId, setSelectedSignalId] = React.useState<string | null>(null);
  const [selectedIds, setSelectedIds] = React.useState<string[]>([]);
  // 削除確認ダイアログの対象。null なら閉じている
  const [pendingDelete, setPendingDelete] = React.useState<TradeHistory[] | null>(null);

  const visibleIds = React.useMemo(() => trades.map((t) => t.id), [trades]);
  // ページ送り・絞り込みで消えた行の選択は落とす
  const selection = React.useMemo(
    () => selectedIds.filter((id) => visibleIds.includes(id)),
    [selectedIds, visibleIds]
  );
  const allSelected = selection.length > 0 && selection.length === visibleIds.length;

  const toggleOne = (id: string, checked: boolean) =>
    setSelectedIds((prev) => (checked ? [...prev, id] : prev.filter((x) => x !== id)));

  const confirmDelete = async () => {
    if (!pendingDelete || !onDelete) return;
    const ids = pendingDelete.map((t) => t.id);
    setPendingDelete(null);
    await onDelete(ids);
    setSelectedIds((prev) => prev.filter((id) => !ids.includes(id)));
  };

  const set = <K extends keyof TradeFilterState>(key: K, value: TradeFilterState[K]) => {
    if (!filter) return;
    filter.onChange({ ...filter.value, [key]: value });
  };

  const toggleSort = (sort: TradeSortKey) => {
    if (!filter) return;
    // 同じ列を再度押したら昇順/降順を反転、別の列なら降順から
    const asc = filter.value.sort === sort ? !filter.value.asc : false;
    filter.onChange({ ...filter.value, sort, asc });
  };

  const activeCount = filter ? countActiveTradeFilters(filter.value) : 0;
  const hasAnyCondition = activeCount > 0 || !!filter?.value.q.trim();

  const formatCurrency = (val: number) => {
    return new Intl.NumberFormat("ja-JP", {
      style: "currency",
      currency: "JPY",
      maximumFractionDigits: 0,
    }).format(val);
  };

  const getCloseReasonBadge = (reason: CloseReason) => {
    switch (reason) {
      case "TAKE_PROFIT":
        return (
          <Badge className="bg-profit/15 text-profit border-profit/30 gap-1 text-[11px]">
            <CheckCircle className="h-3 w-3" />
            利確 (TP)
          </Badge>
        );
      case "STOP_LOSS":
        return (
          <Badge className="bg-loss/15 text-loss border-loss/30 gap-1 text-[11px]">
            <AlertTriangle className="h-3 w-3" />
            損切り (SL)
          </Badge>
        );
      case "STRATEGY":
        return (
          <Badge variant="outline" className="gap-1 text-[11px]">
            <Bot className="h-3 w-3" />
            戦略決済
          </Badge>
        );
      // 以下は LLM 運用時代の旧データ
      case "INVALIDATED":
        return (
          <Badge className="bg-loss/15 text-loss border-loss/30 gap-1 text-[11px]" title="旧データ: 終値判定の損切り">
            <AlertTriangle className="h-3 w-3" />
            旧: 損切り (終値)
          </Badge>
        );
      case "DISCRETIONARY":
        return (
          <Badge variant="outline" className="gap-1 text-[11px] text-muted-foreground" title="旧データ: LLM の判断で決済">
            <BrainCircuit className="h-3 w-3" />
            旧: LLM 判断
          </Badge>
        );
      case "MANUAL":
        return (
          <Badge variant="outline" className="gap-1 text-[11px]">
            <Hand className="h-3 w-3" />
            手動決済
          </Badge>
        );
      case "CIRCUIT_BREAKER":
        return (
          <Badge className="bg-warning/15 text-warning border-warning/30 gap-1 text-[11px]">
            強制停止
          </Badge>
        );
      default:
        return (
          <Badge variant="outline" className="text-[11px] font-mono">
            {reason}
          </Badge>
        );
    }
  };

  /** 並び替え可能な見出し（絞り込みUIがないときはただの見出し） */
  const sortHead = (sort: TradeSortKey, label: string, className?: string) => {
    if (!filter) return <TableHead className={className}>{label}</TableHead>;
    const active = filter.value.sort === sort;
    return (
      <TableHead className={className}>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => toggleSort(sort)}
          aria-label={`${label}で並び替え`}
          className={cn("h-7 px-2 gap-1 text-xs font-medium", !active && "text-muted-foreground")}
        >
          {label}
          {active &&
            (filter.value.asc ? <ArrowUp className="h-3 w-3" /> : <ArrowDown className="h-3 w-3" />)}
        </Button>
      </TableHead>
    );
  };

  // 概要タブ（ページ送り・絞り込みなし）は狭いカラムに置くので、価格と数量の列を省く
  const compact = !pagination && !filter;
  const wideOnly = compact ? "hidden" : "";
  const columnCount = (compact ? 7 : 11) + (onDelete ? 2 : 0);

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-col lg:flex-row lg:items-center justify-between gap-3 pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <History className="h-5 w-5 text-primary" />
            <span>{compact ? "直近の決済" : "取引履歴"}</span>
          </CardTitle>
          <CardDescription className="text-xs">
            決済済みトレードの結果と、管理していた戦略・エントリーしたシグナル
          </CardDescription>
        </div>

        {filter && (
          <div className="flex items-center gap-2 w-full lg:w-auto">
            <div className="relative flex-1 lg:w-64 lg:flex-none">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground pointer-events-none" />
              <Input
                value={filter.value.q}
                onChange={(e) => set("q", e.target.value)}
                placeholder="銘柄・売買・決済理由・シグナルIDを検索"
                aria-label="取引履歴を検索"
                className="h-8 pl-8 pr-8 text-xs"
              />
              {filter.value.q && (
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="検索語をクリア"
                  className="absolute right-0.5 top-1/2 -translate-y-1/2 h-7 w-7"
                  onClick={() => set("q", "")}
                >
                  <X className="h-3.5 w-3.5" />
                </Button>
              )}
            </div>

            <Popover>
              <PopoverTrigger
                render={
                  <Button variant="outline" size="sm" className="h-8 gap-1.5 text-xs shrink-0" />
                }
              >
                <SlidersHorizontal className="h-3.5 w-3.5" />
                絞り込み
                {activeCount > 0 && (
                  <Badge variant="secondary" className="h-4 px-1.5 text-[10px] tabular-nums">
                    {activeCount}
                  </Badge>
                )}
              </PopoverTrigger>
              <PopoverContent align="end" className="w-80 gap-3">
                {filter.symbols && filter.symbols.length > 0 && (
                  <div className="space-y-1.5">
                    <Label className="text-xs text-muted-foreground">銘柄</Label>
                    <Select
                      value={filter.value.symbol}
                      items={{ "": "すべて", ...Object.fromEntries(filter.symbols.map((s) => [s, s])) }}
                      onValueChange={(v) => set("symbol", String(v ?? ""))}
                    >
                      <SelectTrigger size="sm" className="w-full text-xs">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="">すべて</SelectItem>
                        {filter.symbols.map((s) => (
                          <SelectItem key={s} value={s}>
                            {s}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                )}

                {filter.strategies && filter.strategies.length > 0 && (
                  <div className="space-y-1.5">
                    <Label className="text-xs text-muted-foreground">戦略（デプロイ）</Label>
                    <Select
                      value={filter.value.strategy}
                      items={{ "": "すべて", ...Object.fromEntries(filter.strategies.map((s) => [s, s])) }}
                      onValueChange={(v) => set("strategy", String(v ?? ""))}
                    >
                      <SelectTrigger size="sm" className="w-full text-xs">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="">すべて</SelectItem>
                        {filter.strategies.map((s) => (
                          <SelectItem key={s} value={s}>
                            {s}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                )}

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label className="text-xs text-muted-foreground">売買</Label>
                    <Select
                      value={filter.value.side}
                      items={SIDE_LABELS}
                      onValueChange={(v) => set("side", (v ?? "") as TradeFilterState["side"])}
                    >
                      <SelectTrigger size="sm" className="w-full text-xs">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {Object.entries(SIDE_LABELS).map(([value, label]) => (
                          <SelectItem key={value} value={value}>
                            {label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <Label className="text-xs text-muted-foreground">勝敗</Label>
                    <Select
                      value={filter.value.result}
                      items={RESULT_LABELS}
                      onValueChange={(v) => set("result", (v ?? "") as TradeFilterState["result"])}
                    >
                      <SelectTrigger size="sm" className="w-full text-xs">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {Object.entries(RESULT_LABELS).map(([value, label]) => (
                          <SelectItem key={value} value={value}>
                            {label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">決済理由</Label>
                  <Select
                    value={filter.value.closeReason}
                    items={CLOSE_REASON_LABELS}
                    onValueChange={(v) =>
                      set("closeReason", (v ?? "") as TradeFilterState["closeReason"])
                    }
                  >
                    <SelectTrigger size="sm" className="w-full text-xs">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {Object.entries(CLOSE_REASON_LABELS).map(([value, label]) => (
                        <SelectItem key={value} value={value}>
                          {label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label htmlFor="trade-from" className="text-xs text-muted-foreground">
                      決済日（開始）
                    </Label>
                    <Input
                      id="trade-from"
                      type="date"
                      value={filter.value.from}
                      onChange={(e) => set("from", e.target.value)}
                      className="h-8 text-xs"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="trade-to" className="text-xs text-muted-foreground">
                      決済日（終了）
                    </Label>
                    <Input
                      id="trade-to"
                      type="date"
                      value={filter.value.to}
                      onChange={(e) => set("to", e.target.value)}
                      className="h-8 text-xs"
                    />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label htmlFor="trade-min-pips" className="text-xs text-muted-foreground">
                      損益 下限 (pips)
                    </Label>
                    <Input
                      id="trade-min-pips"
                      type="number"
                      inputMode="decimal"
                      value={filter.value.minPnlPips}
                      onChange={(e) => set("minPnlPips", e.target.value)}
                      placeholder="-50"
                      className="h-8 text-xs"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="trade-max-pips" className="text-xs text-muted-foreground">
                      損益 上限 (pips)
                    </Label>
                    <Input
                      id="trade-max-pips"
                      type="number"
                      inputMode="decimal"
                      value={filter.value.maxPnlPips}
                      onChange={(e) => set("maxPnlPips", e.target.value)}
                      placeholder="50"
                      className="h-8 text-xs"
                    />
                  </div>
                </div>

                <Button
                  variant="ghost"
                  size="sm"
                  className="h-8 text-xs"
                  disabled={!hasAnyCondition && filter.value.sort === "closeTime" && !filter.value.asc}
                  onClick={() => filter.onChange({ ...EMPTY_TRADE_FILTER })}
                >
                  条件をすべてクリア
                </Button>
              </PopoverContent>
            </Popover>
          </div>
        )}
      </CardHeader>

      {onDelete && selection.length > 0 && (
        <div className="mx-4 mb-3 flex items-center justify-between gap-3 rounded-lg border bg-muted/40 px-3 py-2">
          <span className="text-xs text-muted-foreground">
            {selection.length} 件を選択中
          </span>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={() => setSelectedIds([])}>
              選択解除
            </Button>
            <Button
              variant="destructive"
              size="sm"
              className="h-7 gap-1 text-xs"
              onClick={() => setPendingDelete(trades.filter((t) => selection.includes(t.id)))}
            >
              <Trash2 className="h-3.5 w-3.5" />
              まとめて削除
            </Button>
          </div>
        </div>
      )}

      <CardContent className="p-0">
        <div className="overflow-x-auto">
          <Table>
            <TableHeader className="bg-muted/40">
              <TableRow>
                {onDelete && (
                  <TableHead className="w-[44px]">
                    <Checkbox
                      checked={allSelected}
                      indeterminate={selection.length > 0 && !allSelected}
                      disabled={visibleIds.length === 0}
                      aria-label="表示中の履歴をすべて選択"
                      onCheckedChange={(checked) => setSelectedIds(checked ? visibleIds : [])}
                    />
                  </TableHead>
                )}
                {sortHead("symbol", "銘柄", "w-[100px]")}
                <TableHead className="w-[80px]">売買</TableHead>
                {!compact && sortHead("volumeLots", "数量", "text-right")}
                {!compact && <TableHead className="text-right">エントリー</TableHead>}
                {!compact && <TableHead className="text-right">決済価格</TableHead>}
                {sortHead("pnlPips", "損益 (pips)", "text-right")}
                {sortHead("pnlAmount", "確定損益", "text-right")}
                <TableHead className="text-center">決済理由</TableHead>
                {!compact && sortHead("closeTime", "決済時刻", "text-right")}
                <TableHead>戦略</TableHead>
                <TableHead className="text-center">シグナル</TableHead>
                {onDelete && <TableHead className="w-[52px] text-center">削除</TableHead>}
              </TableRow>
            </TableHeader>
            <TableBody>
              {trades.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={columnCount}
                    className="h-24 text-center text-muted-foreground text-sm"
                  >
                    {hasAnyCondition
                      ? "条件に一致する取引履歴はありません"
                      : "決済済みのトレード履歴はありません"}
                  </TableCell>
                </TableRow>
              ) : (
                trades.map((trd) => {
                  const isPositive = trd.pnlPips >= 0;
                  return (
                    <TableRow key={trd.id} className="hover:bg-muted/30">
                      {onDelete && (
                        <TableCell>
                          <Checkbox
                            checked={selection.includes(trd.id)}
                            aria-label={`${trd.symbol} ${trd.closeTime} の履歴を選択`}
                            onCheckedChange={(checked) => toggleOne(trd.id, !!checked)}
                          />
                        </TableCell>
                      )}
                      <TableCell className="font-bold text-sm">
                        {trd.symbol}
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant="outline"
                          className={cn(
                            "gap-1 font-semibold text-xs",
                            trd.side === "BUY"
                              ? "border-profit/40 bg-profit/10 text-profit"
                              : "border-loss/40 bg-loss/10 text-loss"
                          )}
                        >
                          {trd.side === "BUY" ? (
                            <ArrowUpRight className="h-3.5 w-3.5" />
                          ) : (
                            <ArrowDownRight className="h-3.5 w-3.5" />
                          )}
                          {trd.side}
                        </Badge>
                      </TableCell>
                      <TableCell className={cn("text-right font-mono tabular-nums text-sm", wideOnly)}>
                        {trd.volumeLots.toFixed(2)} lot
                      </TableCell>
                      <TableCell className={cn("text-right font-mono tabular-nums text-sm", wideOnly)}>
                        {trd.entryPrice.toFixed(priceDigits(trd.symbol))}
                      </TableCell>
                      <TableCell className={cn("text-right font-mono tabular-nums text-sm font-medium", wideOnly)}>
                        {trd.closePrice.toFixed(priceDigits(trd.symbol))}
                      </TableCell>
                      <TableCell
                        className={cn(
                          "text-right font-mono tabular-nums text-sm font-semibold",
                          isPositive
                            ? "text-profit"
                            : "text-loss"
                        )}
                      >
                        {isPositive ? "+" : ""}
                        {trd.pnlPips.toFixed(1)}
                      </TableCell>
                      <TableCell
                        className={cn(
                          "text-right font-mono tabular-nums text-sm font-bold",
                          trd.pnlAmount >= 0
                            ? "text-profit"
                            : "text-loss"
                        )}
                      >
                        {trd.pnlAmount > 0 ? "+" : ""}
                        {formatCurrency(trd.pnlAmount)}
                      </TableCell>
                      <TableCell className="text-center">
                        {getCloseReasonBadge(trd.closeReason)}
                      </TableCell>
                      <TableCell className={cn("text-right text-xs text-muted-foreground font-mono tabular-nums", wideOnly)}>
                        {trd.closeTime}
                      </TableCell>

                      <TableCell>
                        <StrategyBadge strategy={trd.strategy} />
                      </TableCell>

                      <TableCell className="text-center">
                        {isSignalId(trd.signalId) ? (
                          <Button
                            variant="ghost"
                            size="sm"
                            className="h-7 gap-1 text-xs"
                            onClick={() => setSelectedSignalId(trd.signalId ?? null)}
                          >
                            <Radio className="h-3.5 w-3.5" />
                            開く
                          </Button>
                        ) : (
                          <span className="text-xs text-muted-foreground">—</span>
                        )}
                      </TableCell>

                      {onDelete && (
                        <TableCell className="text-center">
                          <Button
                            variant="ghost"
                            size="icon"
                            aria-label={`${trd.symbol} ${trd.closeTime} の履歴を削除`}
                            className="h-7 w-7 text-muted-foreground hover:text-destructive"
                            onClick={() => setPendingDelete([trd])}
                          >
                            <Trash2 className="h-3.5 w-3.5" />
                          </Button>
                        </TableCell>
                      )}
                    </TableRow>
                  );
                })
              )}
            </TableBody>
          </Table>
        </div>
        {pagination && (
          <div className="px-4 py-3 border-t">
            <PaginationBar {...pagination} />
          </div>
        )}
      </CardContent>

      <SignalDetailDialog signalId={selectedSignalId} onClose={() => setSelectedSignalId(null)} />

      <Dialog open={!!pendingDelete} onOpenChange={(open) => !open && setPendingDelete(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              <Trash2 className="h-4 w-4 text-destructive" />
              取引履歴を削除しますか？
            </DialogTitle>
            <DialogDescription className="text-xs">
              {pendingDelete?.length === 1 && pendingDelete[0]
                ? `${pendingDelete[0].symbol} / ${pendingDelete[0].closeTime} の記録を削除します。`
                : `選択した ${pendingDelete?.length ?? 0} 件の記録を削除します。`}
              　削除するのはダッシュボード上の記録のみで、ブローカー側の約定は変わりません。元に戻せません。
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" size="sm" onClick={() => setPendingDelete(null)}>
              キャンセル
            </Button>
            <Button variant="destructive" size="sm" className="gap-1" onClick={confirmDelete}>
              <Trash2 className="h-3.5 w-3.5" />
              削除する
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
