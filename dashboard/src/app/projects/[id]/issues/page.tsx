"use client"

import * as React from "react"
import Link from "next/link"
import { useParams } from "next/navigation"
import { DashboardLayout } from "@/components/layouts/dashboard-layout"
import { IssueList } from "@/components/issues/issue-list"
import { useProject, useIssues } from "@/lib/api"
import { Button, buttonVariants } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { cn } from "@/lib/utils"
import { ArrowLeft, Loader2 } from "lucide-react"

export default function ProjectIssuesPage() {
  const params = useParams()
  const projectId = params.id as string

  const { project, loading: projectLoading } = useProject(projectId)
  const { issues, loading: issuesLoading, refetch: refetchIssues } = useIssues(projectId)

  if (projectLoading) {
    return (
      <DashboardLayout title="課題管理読込中...">
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
          <h3 className="text-base font-semibold mb-2">指定されたプロジェクトが見つかりません</h3>
          <Link href="/projects" className={cn(buttonVariants({ size: "sm" }))}>
            プロジェクト一覧へ
          </Link>
        </Card>
      </DashboardLayout>
    )
  }

  return (
    <DashboardLayout title={`課題管理 — ${project.name}`}>
      <div className="space-y-6">
        <div className="flex items-center gap-3">
          <Link
            href={`/projects/${projectId}`}
            className={cn(buttonVariants({ variant: "ghost", size: "sm" }), "gap-1 px-2 h-8")}
          >
            <ArrowLeft className="h-4 w-4" />
            概要へ戻る
          </Link>
        </div>

        <div>
          <h2 className="text-2xl font-bold tracking-tight">課題管理（Issues）</h2>
          <p className="text-muted-foreground text-sm">
            {project.name} に対する要望・バグ・改善課題の一元管理とAI要件壁打ち。
          </p>
        </div>

        <IssueList
          projectId={projectId}
          issues={issues}
          onIssueCreated={() => refetchIssues()}
        />
      </div>
    </DashboardLayout>
  )
}
