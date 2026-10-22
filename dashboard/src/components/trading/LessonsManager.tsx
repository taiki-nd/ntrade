"use client";

import * as React from "react";
import { LessonLearned } from "@/types/trading";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Lightbulb,
  Plus,
  Trash2,
  Calendar,
  Sparkles,
} from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

/** 教訓の分類（反省のカテゴリと同じ） */
const CATEGORY_LABELS: Record<LessonLearned["category"], string> = {
  PATTERN: "流れ・水準の読み",
  TIMING: "時間帯・注文の期限",
  NEWS: "急変・指標発表",
  RISK: "資金管理・SL 幅",
};

interface LessonsManagerProps {
  lessons: LessonLearned[];
  /** 対象銘柄の選択肢（取引設定の銘柄） */
  pairs?: string[];
  onToggleLesson: (id: string) => void;
  onAddLesson: (newLesson: Omit<LessonLearned, "id" | "createdAt">) => void;
  onDeleteLesson: (id: string) => void;
}

export function LessonsManager({
  lessons,
  pairs = [],
  onToggleLesson,
  onAddLesson,
  onDeleteLesson,
}: LessonsManagerProps) {
  const [openAddDialog, setOpenAddDialog] = React.useState(false);
  const [ruleInput, setRuleInput] = React.useState("");
  const [contextInput, setContextInput] = React.useState("");
  const [symbolInput, setSymbolInput] = React.useState("ALL");
  const [categoryInput, setCategoryInput] = React.useState<LessonLearned["category"]>("PATTERN");

  const handleAddSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!ruleInput.trim()) return;

    onAddLesson({
      rule: ruleInput.trim(),
      context: contextInput.trim() || "手動追加ルール",
      symbol: symbolInput,
      category: categoryInput,
      active: true,
    });

    setRuleInput("");
    setContextInput("");
    setOpenAddDialog(false);
  };

  const getCategoryBadge = (cat: LessonLearned["category"]) => (
    <Badge variant={cat === "RISK" ? "outline" : "secondary"} className="text-[10px]">
      {CATEGORY_LABELS[cat] ?? cat}
    </Badge>
  );
  const symbolItems: Record<string, string> = { ALL: "ALL (全銘柄)", ...Object.fromEntries(pairs.map((p) => [p, p])) };

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-row items-center justify-between pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <Lightbulb className="h-5 w-5 text-warning" />
            <span>教訓</span>
          </CardTitle>
          <CardDescription className="text-xs">
            損切りになった取引を LLM が振り返って候補を作り、同じ分類の候補が 3 回たまると有効になります。有効な教訓は判断のプロンプトに「注意して読む観点」として入ります
          </CardDescription>
        </div>

        <Button
          size="sm"
          onClick={() => setOpenAddDialog(true)}
          className="gap-1 text-xs cursor-pointer"
        >
          <Plus className="h-3.5 w-3.5" />
          教訓を追加
        </Button>

        <Dialog open={openAddDialog} onOpenChange={setOpenAddDialog}>
          <DialogContent className="sm:max-w-md">
            <form onSubmit={handleAddSubmit}>
              <DialogHeader>
                <DialogTitle className="flex items-center gap-2">
                  <Sparkles className="h-4 w-4 text-warning" />
                  新規ルールの追加
                </DialogTitle>
                <DialogDescription>
                  LLM推論プロンプトのコンテキストに注意事項として反映されます。
                </DialogDescription>
              </DialogHeader>
              <div className="space-y-4 py-3">
                <div className="space-y-1.5">
                  <Label className="text-xs font-semibold">教訓（注意して読む観点）</Label>
                  <Textarea
                    placeholder="例: アジア時間の薄い値動きでの高値ブレイクは、逆指値で追う前に 15M の実体で抜けたかを確かめる"
                    value={ruleInput}
                    onChange={(e) => setRuleInput(e.target.value)}
                    rows={3}
                    required
                  />
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label className="text-xs font-semibold">対象銘柄</Label>
                    <Select value={symbolInput} items={symbolItems} onValueChange={(v) => setSymbolInput(String(v ?? "ALL"))}>
                      <SelectTrigger size="sm" className="w-full text-sm">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {Object.entries(symbolItems).map(([v, l]) => (
                          <SelectItem key={v} value={v}>
                            {l}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <Label className="text-xs font-semibold">分類</Label>
                    <Select
                      value={categoryInput}
                      items={CATEGORY_LABELS}
                      onValueChange={(v) => setCategoryInput((v ?? "PATTERN") as LessonLearned["category"])}
                    >
                      <SelectTrigger size="sm" className="w-full text-sm">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {Object.entries(CATEGORY_LABELS).map(([v, l]) => (
                          <SelectItem key={v} value={v}>
                            {l}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs font-semibold">理由・背景</Label>
                  <Input
                    placeholder="例: 期限内に戻されて損切りが続いたため"
                    value={contextInput}
                    onChange={(e) => setContextInput(e.target.value)}
                  />
                </div>
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" onClick={() => setOpenAddDialog(false)}>
                  キャンセル
                </Button>
                <Button type="submit">保存してプロンプトへ反映</Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </CardHeader>
      <CardContent className="space-y-3">
        {lessons.length === 0 ? (
          <div className="h-24 text-center flex items-center justify-center text-muted-foreground text-sm">
            蓄積された教訓ルールはありません
          </div>
        ) : (
          lessons.map((lesson) => (
            <div
              key={lesson.id}
              className={`p-4 rounded-lg border transition-all ${
                lesson.active
                  ? "bg-card border-border"
                  : "bg-muted/30 border-dashed opacity-60"
              }`}
            >
              <div className="flex items-start justify-between gap-4">
                <div className="space-y-1.5 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    {getCategoryBadge(lesson.category)}
                    <Badge variant="outline" className="font-mono text-[10px]">
                      {lesson.symbol}
                    </Badge>
                    <span className="text-xs text-muted-foreground font-mono flex items-center gap-1">
                      <Calendar className="h-3 w-3" />
                      {lesson.createdAt}
                    </span>
                  </div>
                  <p className="text-sm font-semibold leading-snug">
                    {lesson.rule}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    背景: {lesson.context}
                  </p>
                </div>

                <div className="flex items-center gap-3 shrink-0">
                  <div className="flex items-center gap-1.5">
                    <span className="text-xs text-muted-foreground">
                      {lesson.active ? "プロンプトに適用中" : "無効"}
                    </span>
                    <Switch
                      checked={lesson.active}
                      onCheckedChange={() => onToggleLesson(lesson.id)}
                    />
                  </div>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => onDeleteLesson(lesson.id)}
                    className="h-8 w-8 text-muted-foreground hover:text-destructive cursor-pointer"
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            </div>
          ))
        )}
      </CardContent>
    </Card>
  );
}
