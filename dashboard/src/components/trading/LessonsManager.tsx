"use client";

import * as React from "react";
import { LessonLearned } from "@/types/trading";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Lightbulb,
  Plus,
  Trash2,
  AlertCircle,
  Clock,
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
  DialogTrigger,
} from "@/components/ui/dialog";

interface LessonsManagerProps {
  lessons: LessonLearned[];
  onToggleLesson: (id: string) => void;
  onAddLesson: (newLesson: Omit<LessonLearned, "id" | "createdAt">) => void;
  onDeleteLesson: (id: string) => void;
}

export function LessonsManager({
  lessons,
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

  const getCategoryBadge = (cat: LessonLearned["category"]) => {
    switch (cat) {
      case "PATTERN":
        return <Badge variant="secondary" className="text-[10px]">パターン認識</Badge>;
      case "TIMING":
        return <Badge variant="secondary" className="text-[10px]">時間帯・市場</Badge>;
      case "NEWS":
        return <Badge variant="destructive" className="text-[10px]">指標・ニュース</Badge>;
      case "RISK":
        return <Badge variant="outline" className="text-[10px]">資金管理</Badge>;
    }
  };

  return (
    <Card className="shadow-xs">
      <CardHeader className="flex flex-row items-center justify-between pb-3">
        <div className="space-y-1">
          <CardTitle className="text-base font-semibold flex items-center gap-2">
            <Lightbulb className="h-5 w-5 text-amber-500" />
            <span>AI 自己反省 & 教訓マネージャー (Lessons Learned)</span>
          </CardTitle>
          <CardDescription className="text-xs">
            損切り・失敗トレードの自己反省（Self-Reflection）から自動蓄積され、次回の推論プロンプトに注入される厳格ルール
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
                  <Sparkles className="h-4 w-4 text-amber-500" />
                  新規ルールの追加
                </DialogTitle>
                <DialogDescription>
                  LLM推論プロンプトのコンテキストに注意事項として反映されます。
                </DialogDescription>
              </DialogHeader>
              <div className="space-y-4 py-3">
                <div className="space-y-1.5">
                  <label className="text-xs font-semibold">ルール本文（禁止事項・注意）</label>
                  <Textarea
                    placeholder="例: 4H上位足が上昇トレンドの局面では、5M逆張りショートは厳禁。"
                    value={ruleInput}
                    onChange={(e) => setRuleInput(e.target.value)}
                    rows={3}
                    required
                  />
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-xs font-semibold">対象通貨ペア</label>
                    <select
                      className="w-full rounded-md border bg-background px-3 py-1.5 text-sm"
                      value={symbolInput}
                      onChange={(e) => setSymbolInput(e.target.value)}
                    >
                      <option value="ALL">ALL (全銘柄)</option>
                      <option value="USDJPY">USDJPY</option>
                      <option value="EURUSD">EURUSD</option>
                    </select>
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-xs font-semibold">カテゴリ</label>
                    <select
                      className="w-full rounded-md border bg-background px-3 py-1.5 text-sm"
                      value={categoryInput}
                      onChange={(e) => setCategoryInput(e.target.value as any)}
                    >
                      <option value="PATTERN">パターン認識</option>
                      <option value="TIMING">時間帯・市場</option>
                      <option value="NEWS">指標・ニュース</option>
                      <option value="RISK">資金管理</option>
                    </select>
                  </div>
                </div>
                <div className="space-y-1.5">
                  <label className="text-xs font-semibold">理由・背景（コンテキスト）</label>
                  <Input
                    placeholder="例: 踏み上げリスクが高く損切り多発のため"
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
                      {lesson.active ? "プロンプト注入中" : "無効"}
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
