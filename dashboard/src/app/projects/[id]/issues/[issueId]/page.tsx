"use client"

import * as React from "react"
import Link from "next/link"
import { useParams } from "next/navigation"
import { DashboardLayout } from "@/components/layouts/dashboard-layout"
import { IssueDiffViewer } from "@/components/issues/issue-diff-viewer"
import { IssueChatThread } from "@/components/issues/issue-chat-thread"
import { useProject, useIssue, useIssueComments } from "@/lib/api"
import { Button, buttonVariants } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { cn } from "@/lib/utils"
import { ArrowLeft, GitPullRequest, Loader2 } from "lucide-react"

export default function IssueDetailPage() {
  const params = useParams()
  const projectId = params.id as string
  const issueId = params.issueId as string

  const { project, loading: projectLoading } = useProject(projectId)
  const { issue, loading: issueLoading } = useIssue(issueId)
  const {
    comments,
    sendChatMessage,
    triggerExecuteFix,
  } = useIssueComments(issueId)

  if (projectLoading || issueLoading) {
    return (
      <DashboardLayout title="課題詳細読込中...">
        <div className="flex h-64 items-center justify-center">
          <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
        </div>
      </DashboardLayout>
    )
  }

  if (!issue) {
    return (
      <DashboardLayout title="課題が見つかりません">
        <Card className="p-8 text-center bg-muted/20">
          <h3 className="text-base font-semibold mb-2">指定された課題は見つかりませんでした</h3>
          <Link href={`/projects/${projectId}/issues`} className={cn(buttonVariants({ size: "sm" }))}>
            課題一覧へ戻る
          </Link>
        </Card>
      </DashboardLayout>
    )
  }

  const projectName = project?.name || "プロジェクト"

  return (
    <DashboardLayout title={`#${issue.id.slice(-6)} ${issue.title}`}>
      <div className="space-y-4">
        {/* ナビゲーションバー */}
        <div className="flex items-center justify-between">
          <Link
            href={`/projects/${projectId}/issues`}
            className={cn(buttonVariants({ variant: "ghost", size: "sm" }), "gap-1.5 px-2 h-8")}
          >
            <ArrowLeft className="h-4 w-4" />
            課題一覧へ戻る
          </Link>

          <div className="flex items-center gap-2 text-xs text-muted-foreground font-mono">
            <span>{projectName}</span>
            <span>/</span>
            <span className="text-foreground font-semibold">
              Issue #{issue.id.slice(-6)}
            </span>
          </div>
        </div>

        {/* 左右 2 カラムレイアウト */}
        <div className="grid gap-6 lg:grid-cols-12 items-start">
          {/* 左カラム (5/12): 課題仕様・プレビュー・Diffサマリ */}
          <div className="lg:col-span-5 space-y-4">
            <IssueDiffViewer issue={issue} />
          </div>

          {/* 右カラム (7/12): AI 壁打ちスレッド & 修正実行 */}
          <div className="lg:col-span-7">
            <IssueChatThread
              issueId={issue.id}
              initialComments={comments}
              onSendMessage={sendChatMessage}
              onTriggerFix={triggerExecuteFix}
            />
          </div>
        </div>
      </div>
    </DashboardLayout>
  )
}
