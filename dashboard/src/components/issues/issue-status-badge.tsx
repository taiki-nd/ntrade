import * as React from "react"
import { Badge } from "@/components/ui/badge"
import { IssueStatus } from "@/types/schema"
import { MessageSquare, Wrench, CheckCircle2, CircleDot, Archive } from "lucide-react"

interface IssueStatusBadgeProps {
  status: IssueStatus
  className?: string
}

export function IssueStatusBadge({ status, className }: IssueStatusBadgeProps) {
  switch (status) {
    case "open":
      return (
        <Badge variant="outline" className={className}>
          <CircleDot className="mr-1 h-3 w-3 text-muted-foreground" />
          未対応
        </Badge>
      )
    case "chatting":
      return (
        <Badge variant="secondary" className={className}>
          <MessageSquare className="mr-1 h-3 w-3 text-foreground" />
          壁打ち中
        </Badge>
      )
    case "fixing":
      return (
        <Badge variant="default" className={className}>
          <Wrench className="mr-1 h-3 w-3" />
          自律修正中
        </Badge>
      )
    case "review_ready":
      return (
        <Badge variant="outline" className={`bg-accent text-accent-foreground border-border ${className || ""}`}>
          <CheckCircle2 className="mr-1 h-3 w-3" />
          検収待ち
        </Badge>
      )
    case "closed":
      return (
        <Badge variant="outline" className={`text-muted-foreground opacity-70 ${className || ""}`}>
          <Archive className="mr-1 h-3 w-3" />
          完了
        </Badge>
      )
  }
}
