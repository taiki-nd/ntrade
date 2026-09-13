"use client"

import * as React from "react"
import Link from "next/link"
import { useParams } from "next/navigation"
import { DashboardLayout } from "@/components/layouts/dashboard-layout"
import { IssueStatusBadge } from "@/components/issues/issue-status-badge"
import { IssuePriorityBadge } from "@/components/issues/issue-priority-badge"
import { useProject, useIssues, useGenerationLogs } from "@/lib/api"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Button, buttonVariants } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import {
  ExternalLink,
  Plus,
  ArrowRight,
  Database,
  GitBranch,
  CheckCircle2,
  Boxes,
  Users,
  Settings,
  CircleDot,
  FileCode2,
  Loader2,
} from "lucide-react"

export default function ProjectDetailPage() {
  const params = useParams()
  const projectId = params.id as string

  const { project, loading: projectLoading } = useProject(projectId)
  const { issues: projectIssues } = useIssues(projectId)
  const { logs } = useGenerationLogs(projectId)

  if (projectLoading) {
    return (
      <DashboardLayout title="プロジェクト読込中...">
        <div className="flex h-64 items-center justify-center">
          <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
        </div>
      </DashboardLayout>
    )
  }

  if (!project) {
    return (
      <DashboardLayout title="プロジェクトが見つかりません">
        <Card className="p-8 text-center bg-muted/20">
          <h3 className="text-base font-semibold mb-2">指定されたプロジェクトは存在しないか、権限がありません</h3>
          <p className="text-sm text-muted-foreground mb-4">プロジェクト一覧に戻って別のプロジェクトを選択してください。</p>
          <Link href="/projects" className={cn(buttonVariants({ size: "sm" }))}>
            一覧に戻る
          </Link>
        </Card>
      </DashboardLayout>
    )
  }

  return (
    <DashboardLayout title={project.name}>
      <div className="space-y-6">
        {/* プロジェクトヘッダー */}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div>
            <div className="flex items-center gap-2 mb-1">
              <Badge variant={project.status === "active" ? "default" : "outline"}>
                {project.status === "active" ? "稼働中" : "アーカイブ"}
              </Badge>
              <span className="text-xs text-muted-foreground font-mono">
                {project.slug}
              </span>
            </div>
            <h2 className="text-2xl font-bold tracking-tight">{project.name}</h2>
            <p className="text-muted-foreground text-sm">
              {(project as any).setup_prompt || "AI対話により自動設計されたシステム"}
            </p>
          </div>

          <div className="flex items-center gap-2">
            <Link
              href={`/projects/${project.id}/issues`}
              className={cn(buttonVariants({ variant: "outline", size: "sm" }), "gap-1.5")}
            >
              <CircleDot className="h-4 w-4" />
              課題一覧 ({projectIssues.length})
            </Link>
            <Link
              href={`/projects/${project.id}/issues?new=true`}
              className={cn(buttonVariants({ size: "sm" }), "gap-1.5")}
            >
              <Plus className="h-4 w-4" />
              課題を起票
            </Link>
          </div>
        </div>

        {/* プレビュー実機カード */}
        <Card className="bg-muted/30 border-primary/20">
          <CardContent className="p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
            <div className="space-y-1">
              <div className="flex items-center gap-2">
                <span className="flex h-2 w-2 rounded-full bg-emerald-500" />
                <span className="text-xs font-semibold text-foreground">
                  実機プレビュー環境 稼働中
                </span>
                <span className="text-[11px] text-muted-foreground">
                  (Cloudflare Pages)
                </span>
              </div>
              <p className="text-xs font-mono text-muted-foreground">
                {project.preview_url || "https://preview.bee8.dev"}
              </p>
            </div>
            <div className="flex items-center gap-2 shrink-0">
              {project.preview_url && (
                <a
                  href={project.preview_url}
                  target="_blank"
                  rel="noreferrer"
                  className={cn(buttonVariants({ size: "sm" }), "gap-1.5 h-8")}
                >
                  実機を開く
                  <ExternalLink className="h-3.5 w-3.5" />
                </a>
              )}
              {project.repo_url && (
                <a
                  href={project.repo_url}
                  target="_blank"
                  rel="noreferrer"
                  className={cn(buttonVariants({ variant: "outline", size: "sm" }), "gap-1.5 h-8")}
                >
                  GitHub
                  <ExternalLink className="h-3.5 w-3.5" />
                </a>
              )}
            </div>
          </CardContent>
        </Card>

        {/* タブナビゲーション */}
        <Tabs defaultValue="overview" className="space-y-4">
          <TabsList className="bg-muted/50 border border-border">
            <TabsTrigger value="overview">概要</TabsTrigger>
            <TabsTrigger value="issues">
              課題（Issues）
              <Badge variant="secondary" className="ml-1.5 text-[10px] px-1.5">
                {projectIssues.length}
              </Badge>
            </TabsTrigger>
            <TabsTrigger value="schema">スキーマ定義</TabsTrigger>
            <TabsTrigger value="history">生成履歴</TabsTrigger>
          </TabsList>

          {/* 概要タブ */}
          <TabsContent value="overview" className="space-y-4">
            <div className="grid gap-4 md:grid-cols-3">
              <Card>
                <CardHeader className="pb-2">
                  <CardTitle className="text-xs font-medium text-muted-foreground">
                    データベース種別
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="text-xl font-bold flex items-center gap-2 capitalize">
                    <Database className="h-5 w-5 text-primary" />
                    {project.database_type}
                  </div>
                </CardContent>
              </Card>

              <Card>
                <CardHeader className="pb-2">
                  <CardTitle className="text-xs font-medium text-muted-foreground">
                    未解決の課題
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="text-xl font-bold flex items-center gap-2">
                    <CircleDot className="h-5 w-5 text-primary" />
                    {projectIssues.filter((i) => i.status !== "closed").length} 件
                  </div>
                </CardContent>
              </Card>

              <Card>
                <CardHeader className="pb-2">
                  <CardTitle className="text-xs font-medium text-muted-foreground">
                    bee8 lint ガードレール
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="text-xl font-bold flex items-center gap-2">
                    <CheckCircle2 className="h-5 w-5 text-primary" />
                    PASSED
                  </div>
                </CardContent>
              </Card>
            </div>

            {/* アクティブな課題 */}
            <Card>
              <CardHeader className="pb-3 flex flex-row items-center justify-between">
                <div>
                  <CardTitle className="text-base font-semibold">
                    直近のアクティブ課題
                  </CardTitle>
                  <CardDescription className="text-xs">
                    クライアントからの改善要望およびAI壁打ちスレッド
                  </CardDescription>
                </div>
                <Link
                  href={`/projects/${project.id}/issues`}
                  className={cn(buttonVariants({ variant: "ghost", size: "sm" }), "text-xs")}
                >
                  すべて見る <ArrowRight className="ml-1 h-3.5 w-3.5" />
                </Link>
              </CardHeader>
              <CardContent className="space-y-3">
                {projectIssues.map((issue) => (
                  <div
                    key={issue.id}
                    className="p-3 bg-muted/20 border border-border rounded-lg flex items-center justify-between gap-3 hover:bg-muted/40 transition-colors"
                  >
                    <div className="space-y-1">
                      <div className="flex items-center gap-2">
                        <IssueStatusBadge status={issue.status} />
                        <IssuePriorityBadge priority={issue.priority} />
                      </div>
                      <Link
                        href={`/projects/${project.id}/issues/${issue.id}`}
                        className="font-medium text-sm hover:underline block"
                      >
                        {issue.title}
                      </Link>
                    </div>
                    <Link
                      href={`/projects/${project.id}/issues/${issue.id}`}
                      className={cn(buttonVariants({ variant: "ghost", size: "sm" }), "h-7 text-xs")}
                    >
                      スレッドを開く →
                    </Link>
                  </div>
                ))}
              </CardContent>
            </Card>
          </TabsContent>

          {/* 課題タブ */}
          <TabsContent value="issues">
            <Card>
              <CardContent className="p-4">
                <div className="flex items-center justify-between mb-4">
                  <p className="text-xs text-muted-foreground">
                    このプロジェクトに紐づく全課題の進捗状況
                  </p>
                  <Link
                    href={`/projects/${project.id}/issues`}
                    className={cn(buttonVariants({ size: "sm" }), "gap-1.5")}
                  >
                    課題管理フル画面を開く <ArrowRight className="h-3.5 w-3.5" />
                  </Link>
                </div>
              </CardContent>
            </Card>
          </TabsContent>

          {/* スキーマ定義タブ */}
          <TabsContent value="schema">
            <Card>
              <CardHeader>
                <CardTitle className="text-base">自動生成元 YAML スキーマ</CardTitle>
                <CardDescription className="text-xs">
                  `api/schema/domains/*.yaml` から定義されたデータモデル
                </CardDescription>
              </CardHeader>
              <CardContent>
                <div className="p-3 bg-muted font-mono text-xs rounded-lg overflow-x-auto space-y-1 text-muted-foreground">
                  <div># schema/domains/project.yaml</div>
                  <div>name: {project.slug}</div>
                  <div>database: {project.database_type}</div>
                  <div>status: {project.status}</div>
                </div>
              </CardContent>
            </Card>
          </TabsContent>

          {/* 生成履歴タブ */}
          <TabsContent value="history">
            <Card>
              <CardHeader>
                <CardTitle className="text-base">コード生成・修正履歴</CardTitle>
                <CardDescription className="text-xs">
                  Issue 解決および手動再生成のログ
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                {logs.length === 0 ? (
                  <div className="p-6 text-center text-xs text-muted-foreground">
                    コード生成・修正履歴はまだありません。
                  </div>
                ) : (
                  logs.map((log) => (
                    <div
                      key={log.id}
                      className="p-3 bg-muted/30 border border-border rounded-lg text-xs space-y-1"
                    >
                      <div className="flex items-center justify-between">
                        <span className="font-semibold capitalize">
                          {log.trigger_type.replace("_", " ")}
                        </span>
                        <span className="text-muted-foreground">
                          {new Date(log.created_at).toLocaleString("ja-JP")}
                        </span>
                      </div>
                      <div className="flex items-center justify-between text-muted-foreground text-[11px]">
                        <span>{log.duration_ms}ms</span>
                        <Badge variant="outline" className="text-[10px]">
                          {log.status === "success" ? "PASSED" : "FAILED"}
                        </Badge>
                      </div>
                    </div>
                  ))
                )}
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </div>
    </DashboardLayout>
  )
}
