import * as React from "react"
import { Badge } from "@/components/ui/badge"
import { IssuePriority } from "@/types/schema"

interface IssuePriorityBadgeProps {
  priority: IssuePriority
  className?: string
}

export function IssuePriorityBadge({ priority, className }: IssuePriorityBadgeProps) {
  switch (priority) {
    case "low":
      return (
        <Badge variant="outline" className={`text-muted-foreground ${className || ""}`}>
          低
        </Badge>
      )
    case "medium":
      return (
        <Badge variant="outline" className={className}>
          中
        </Badge>
      )
    case "high":
      return (
        <Badge variant="secondary" className={className}>
          高
        </Badge>
      )
    case "urgent":
      return (
        <Badge variant="destructive" className={className}>
          緊急
        </Badge>
      )
  }
}
