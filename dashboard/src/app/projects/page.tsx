"use client"

import * as React from "react"
import { DashboardLayout } from "@/components/layouts/dashboard-layout"
import { ProjectList } from "@/components/projects/project-list"
import { useProjects } from "@/lib/api"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { RefreshCw } from "lucide-react"

export default function ProjectsPage() {
  const { projects, loading, isLive, refetch } = useProjects()

  return (
    <DashboardLayout title="プロジェクト一覧">
      <div className="space-y-6">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-3">
              <h2 className="text-2xl font-bold tracking-tight">プロジェクト一覧</h2>
            </div>
            <p className="text-muted-foreground text-sm">
              管理対象のプロダクトおよび受託クライアント環境の一覧。
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={() => refetch()} disabled={loading} className="gap-1.5">
            <RefreshCw className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
            更新
          </Button>
        </div>

        <ProjectList projects={projects} />
      </div>
    </DashboardLayout>
  )
}
