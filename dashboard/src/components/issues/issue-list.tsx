"use client"

import * as React from "react"
import Link from "next/link"
import { Issue, IssueStatus } from "@/types/schema"
import { IssueStatusBadge } from "./issue-status-badge"
import { IssuePriorityBadge } from "./issue-priority-badge"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Plus, Search, ExternalLink, Loader2 } from "lucide-react"
import { createIssue } from "@/lib/api"
import type { IssuePriority } from "@/types/schema"

interface IssueListProps {
  projectId: string
  issues: Issue[]
  onIssueCreated?: (issue: Issue) => void
}

export function IssueList({ projectId, issues, onIssueCreated }: IssueListProps) {
  const [searchTerm, setSearchTerm] = React.useState("")
  const [statusFilter, setStatusFilter] = React.useState<string>("all")
  const [dialogOpen, setDialogOpen] = React.useState(false)
  const [submitting, setSubmitting] = React.useState(false)
  const [title, setTitle] = React.useState("")
  const [description, setDescription] = React.useState("")
  const [priority, setPriority] = React.useState<IssuePriority>("medium")
  const [localIssues, setLocalIssues] = React.useState<Issue[]>(issues)

  React.useEffect(() => {
    setLocalIssues(issues)
  }, [issues])

  const handleCreateIssue = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!title.trim() || !description.trim()) return

    setSubmitting(true)
    try {
      const created = await createIssue({
        projectId,
        title: title.trim(),
        description: description.trim(),
        priority,
      })
      setLocalIssues((prev) => [created, ...prev])
      onIssueCreated?.(created)
      setDialogOpen(false)
      setTitle("")
      setDescription("")
      setPriority("medium")
    } catch (err: any) {
      console.error("Failed to create issue:", err)
      // Local fallback for offline/preview
      const fallback: Issue = {
        id: `issue_${Date.now()}`,
        project_id: projectId,
        created_by_id: "user_current",
        title: title.trim(),
        description: description.trim(),
        status: "open",
        priority,
        created_at: new Date().toISOString(),
      }
      setLocalIssues((prev) => [fallback, ...prev])
      onIssueCreated?.(fallback)
      setDialogOpen(false)
      setTitle("")
      setDescription("")
      setPriority("medium")
    } finally {
      setSubmitting(false)
    }
  }

  const filteredIssues = localIssues.filter((issue) => {
    const matchesSearch =
      issue.title.toLowerCase().includes(searchTerm.toLowerCase()) ||
      (issue.description || "").toLowerCase().includes(searchTerm.toLowerCase())
    const matchesStatus =
      statusFilter === "all" || issue.status === statusFilter
    return matchesSearch && matchesStatus
  })

  return (
    <div className="space-y-4">
      {/* フィルタ & 検索バー */}
      <div className="flex flex-col sm:flex-row items-center justify-between gap-3">
        <div className="relative w-full sm:w-80">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="課題を検索..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="pl-9"
          />
        </div>

        <div className="flex items-center gap-2 w-full sm:w-auto justify-between sm:justify-end">
          <div className="flex items-center gap-1 bg-muted/50 p-1 rounded-lg border border-border">
            {(["all", "open", "chatting", "review_ready", "closed"] as const).map((st) => (
              <Button
                key={st}
                variant={statusFilter === st ? "secondary" : "ghost"}
                size="sm"
                className="h-7 text-xs px-2.5"
                onClick={() => setStatusFilter(st)}
              >
                {st === "all" && "すべて"}
                {st === "open" && "未対応"}
                {st === "chatting" && "壁打ち中"}
                {st === "review_ready" && "検収待ち"}
                {st === "closed" && "完了"}
              </Button>
            ))}
          </div>

          <Button size="sm" className="gap-1.5 shrink-0" onClick={() => setDialogOpen(true)}>
            <Plus className="h-4 w-4" />
            課題を追加
          </Button>
        </div>
      </div>

      {/* 新規課題作成ダイアログ */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="sm:max-w-[520px]">
          <form onSubmit={handleCreateIssue}>
            <DialogHeader>
              <DialogTitle>新規課題の起票</DialogTitle>
              <DialogDescription>
                プロダクトに対するバグ報告、仕様変更、新機能要望を登録します。AIエージェントが要件の壁打ち対話をサポートします。
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-4 py-4">
              <div className="space-y-2">
                <Label htmlFor="issue-title">タイトル *</Label>
                <Input
                  id="issue-title"
                  placeholder="例: 商品一覧にカテゴリ絞り込み機能を追加したい"
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  required
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="issue-priority">優先度</Label>
                <Select value={priority} onValueChange={(val) => setPriority(val as IssuePriority)}>
                  <SelectTrigger id="issue-priority">
                    <SelectValue placeholder="優先度を選択" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="low">低 (Low)</SelectItem>
                    <SelectItem value="medium">中 (Medium)</SelectItem>
                    <SelectItem value="high">高 (High)</SelectItem>
                    <SelectItem value="urgent">緊急 (Urgent)</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <Label htmlFor="issue-desc">詳細・要求内容 *</Label>
                <Textarea
                  id="issue-desc"
                  placeholder="現状の動作と、期待する仕様を記載してください。"
                  rows={4}
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  required
                />
              </div>
            </div>

            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setDialogOpen(false)}>
                キャンセル
              </Button>
              <Button type="submit" disabled={submitting || !title.trim() || !description.trim()}>
                {submitting && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                課題を起票
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      {/* 課題一覧テーブル */}
      <div className="rounded-lg border border-border bg-card">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[120px]">ステータス</TableHead>
              <TableHead className="w-[80px]">優先度</TableHead>
              <TableHead>課題タイトル</TableHead>
              <TableHead className="w-[180px] hidden md:table-cell">プレビュー / ブランチ</TableHead>
              <TableHead className="w-[130px] text-right">更新日時</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {filteredIssues.length === 0 ? (
              <TableRow>
                <TableCell colSpan={5} className="h-24 text-center text-muted-foreground">
                  該当する課題が見つかりません。
                </TableCell>
              </TableRow>
            ) : (
              filteredIssues.map((issue) => (
                <TableRow key={issue.id} className="cursor-pointer hover:bg-muted/40">
                  <TableCell>
                    <IssueStatusBadge status={issue.status} />
                  </TableCell>
                  <TableCell>
                    <IssuePriorityBadge priority={issue.priority} />
                  </TableCell>
                  <TableCell>
                    <Link
                      href={`/projects/${projectId}/issues/${issue.id}`}
                      className="font-medium hover:underline block"
                    >
                      {issue.title}
                    </Link>
                    <p className="text-xs text-muted-foreground line-clamp-1 mt-0.5">
                      {issue.description}
                    </p>
                  </TableCell>
                  <TableCell className="hidden md:table-cell">
                    {issue.preview_url ? (
                      <a
                        href={issue.preview_url}
                        target="_blank"
                        rel="noreferrer"
                        className="inline-flex items-center text-xs text-muted-foreground hover:text-foreground gap-1"
                        onClick={(e) => e.stopPropagation()}
                      >
                        実機プレビュー
                        <ExternalLink className="h-3 w-3" />
                      </a>
                    ) : (
                      <span className="text-xs text-muted-foreground">未デプロイ</span>
                    )}
                  </TableCell>
                  <TableCell className="text-right text-xs text-muted-foreground">
                    {new Date(issue.updated_at || issue.created_at).toLocaleDateString("ja-JP", {
                      month: "short",
                      day: "numeric",
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  )
}
