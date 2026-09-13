"use client"

import * as React from "react"
import Link from "next/link"
import { Project } from "@/types/schema"
import { ProjectCard } from "./project-card"
import { Input } from "@/components/ui/input"
import { Button, buttonVariants } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { Plus, Search } from "lucide-react"

interface ProjectListProps {
  projects: Project[]
}

export function ProjectList({ projects }: ProjectListProps) {
  const [searchTerm, setSearchTerm] = React.useState("")

  const filtered = projects.filter((p) =>
    p.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
    p.slug.toLowerCase().includes(searchTerm.toLowerCase())
  )

  return (
    <div className="space-y-4">
      <div className="flex flex-col sm:flex-row items-center justify-between gap-3">
        <div className="relative w-full sm:w-80">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="プロジェクトを検索..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="pl-9"
          />
        </div>

        <Link
          href="/projects/new"
          className={cn(buttonVariants({ size: "sm" }), "gap-1.5 w-full sm:w-auto")}
        >
          <Plus className="h-4 w-4" />
          新規プロジェクト作成
        </Link>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {filtered.map((project) => (
          <ProjectCard key={project.id} project={project} />
        ))}
      </div>

      {filtered.length === 0 && (
        <div className="p-8 text-center text-muted-foreground border border-dashed border-border rounded-lg">
          該当するプロジェクトは見つかりませんでした。
        </div>
      )}
    </div>
  )
}
