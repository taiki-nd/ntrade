import * as React from "react"
import Link from "next/link"
import { Project } from "@/types/schema"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button, buttonVariants } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { Database, ExternalLink, GitBranch, FolderKanban } from "lucide-react"

interface ProjectCardProps {
  project: Project
}

export function ProjectCard({ project }: ProjectCardProps) {
  const isArchived = project.status === "archived"

  return (
    <Card className="hover:border-primary/50 transition-colors flex flex-col justify-between">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between gap-2 mb-1">
          <div className="flex items-center gap-2">
            <Badge variant={isArchived ? "outline" : "default"} className="text-xs capitalize">
              {project.status === "active" ? "稼働中" : "アーカイブ"}
            </Badge>
            <span className="text-xs text-muted-foreground flex items-center gap-1">
              <Database className="h-3 w-3" />
              {project.database_type}
            </span>
          </div>
          <span className="text-[11px] text-muted-foreground">
            {new Date(project.updated_at || project.created_at).toLocaleDateString("ja-JP")}
          </span>
        </div>

        <CardTitle className="text-base font-semibold leading-snug">
          <Link href={`/projects/${project.id}`} className="hover:underline">
            {project.name}
          </Link>
        </CardTitle>
        <CardDescription className="text-xs line-clamp-2">
          {project.setup_prompt || "自然言語要件定義から生成されたシステム"}
        </CardDescription>
      </CardHeader>

      <CardContent className="pt-0 space-y-3">
        <div className="flex items-center justify-between pt-2 border-t border-border text-xs">
          <div className="flex items-center gap-2">
            {project.preview_url ? (
              <a
                href={project.preview_url}
                target="_blank"
                rel="noreferrer"
                className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1"
              >
                プレビュー
                <ExternalLink className="h-3 w-3" />
              </a>
            ) : (
              <span className="text-muted-foreground">プレビューなし</span>
            )}
            {project.repo_url && (
              <a
                href={project.repo_url}
                target="_blank"
                rel="noreferrer"
                className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1"
              >
                GitHub
                <ExternalLink className="h-3 w-3" />
              </a>
            )}
          </div>

          <Link
            href={`/projects/${project.id}`}
            className={cn(buttonVariants({ variant: "ghost", size: "sm" }), "h-7 text-xs px-2")}
          >
            開く →
          </Link>
        </div>
      </CardContent>
    </Card>
  )
}
