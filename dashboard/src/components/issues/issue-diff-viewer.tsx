"use client"

import * as React from "react"
import { Issue } from "@/types/schema"
import { IssueStatusBadge } from "./issue-status-badge"
import { IssuePriorityBadge } from "./issue-priority-badge"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Button, buttonVariants } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { Separator } from "@/components/ui/separator"
import { ExternalLink, Check, GitCommit, FileCode, CheckCircle2 } from "lucide-react"

interface IssueDiffViewerProps {
  issue: Issue
  onResolve?: () => void
}

export function IssueDiffViewer({ issue, onResolve }: IssueDiffViewerProps) {
  const [resolved, setResolved] = React.useState(issue.status === "closed")

  const handleResolve = () => {
    setResolved(true)
    onResolve?.()
  }

  return (
    <div className="space-y-4">
      {/* 課題基本情報カード */}
      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <IssueStatusBadge status={resolved ? "closed" : issue.status} />
              <IssuePriorityBadge priority={issue.priority} />
            </div>
            {issue.branch_name && (
              <span className="text-xs font-mono text-muted-foreground flex items-center gap-1 bg-muted px-2 py-0.5 rounded">
                <GitCommit className="h-3 w-3" />
                {issue.branch_name}
              </span>
            )}
          </div>
          <CardTitle className="text-lg font-bold mt-2 leading-tight">
            {issue.title}
          </CardTitle>
          <CardDescription className="text-xs">
            起票日: {new Date(issue.created_at).toLocaleDateString("ja-JP")}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4 text-sm">
          <div className="p-3 bg-muted/40 rounded-lg text-muted-foreground whitespace-pre-wrap leading-relaxed">
            {issue.description}
          </div>

          <Separator />

          {/* 実機プレビュー連携枠 */}
          <div>
            <div className="text-xs font-semibold text-foreground mb-1.5 flex items-center justify-between">
              <span>プレビュー環境</span>
              {issue.preview_url && (
                <span className="text-[10px] text-muted-foreground font-normal">
                  自動デプロイ済み
                </span>
              )}
            </div>
            {issue.preview_url ? (
              <div className="flex items-center justify-between p-2.5 bg-muted/30 border border-border rounded-lg">
                <div className="truncate text-xs font-mono text-muted-foreground mr-2">
                  {issue.preview_url}
                </div>
                <a
                  href={issue.preview_url}
                  target="_blank"
                  rel="noreferrer"
                  className={cn(buttonVariants({ variant: "outline", size: "sm" }), "h-7 text-xs gap-1")}
                >
                  実機を開く
                  <ExternalLink className="h-3 w-3" />
                </a>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground p-2.5 bg-muted/20 border border-dashed border-border rounded-lg text-center">
                対話で仕様確定後、「修正を実行」すると自動起動します
              </p>
            )}
          </div>

          <Separator />

          {/* 変更差分サマリ */}
          <div>
            <div className="text-xs font-semibold text-foreground mb-2 flex items-center gap-1.5">
              <FileCode className="h-3.5 w-3.5 text-muted-foreground" />
              変更差分（Diff）サマリ
            </div>
            {issue.diff_summary ? (
              <div className="space-y-2 p-3 bg-muted/30 border border-border rounded-lg text-xs">
                <div className="flex items-center gap-3">
                  <span className="font-medium text-foreground">
                    {issue.diff_summary.files_changed} ファイル変更
                  </span>
                  <span className="font-mono text-foreground/80">
                    +{issue.diff_summary.additions} / -{issue.diff_summary.deletions} 行
                  </span>
                </div>
                <p className="text-muted-foreground">
                  {issue.diff_summary.summary}
                </p>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">
                まだコード修正は実行されていません。
              </p>
            )}
          </div>

          <Separator />

          {/* 検収アクション */}
          <div className="pt-1">
            <Button
              variant={resolved ? "outline" : "default"}
              className="w-full gap-1.5"
              disabled={resolved}
              onClick={handleResolve}
            >
              {resolved ? (
                <>
                  <CheckCircle2 className="h-4 w-4" />
                  検収完了済み（Closed）
                </>
              ) : (
                <>
                  <Check className="h-4 w-4" />
                  プレビュー確認済み・検収完了とする
                </>
              )}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
