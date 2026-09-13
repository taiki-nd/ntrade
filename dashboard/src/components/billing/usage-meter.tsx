import * as React from "react"
import { Subscription } from "@/types/schema"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Zap, Boxes, ExternalLink, ArrowUpRight } from "lucide-react"

interface UsageMeterProps {
  subscription: Subscription
}

export function UsageMeter({ subscription }: UsageMeterProps) {
  const genPercent = Math.min(
    100,
    Math.round((subscription.monthly_generations_used / subscription.monthly_generations_limit) * 100)
  )
  const projPercent = Math.min(
    100,
    Math.round((subscription.projects_used / subscription.projects_limit) * 100)
  )

  return (
    <div className="grid gap-4 sm:grid-cols-2">
      {/* 生成回数メーター */}
      <Card>
        <CardHeader className="pb-2">
          <div className="flex items-center justify-between">
            <CardTitle className="text-sm font-medium flex items-center gap-1.5">
              <Zap className="h-4 w-4 text-primary" />
              今月の生成回数
            </CardTitle>
            <Badge variant="secondary" className="uppercase text-[10px]">
              {subscription.plan} プラン
            </Badge>
          </div>
          <CardDescription className="text-xs">
            課題修正・ASTコード生成・API同期
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-2">
          <div className="flex items-baseline justify-between text-sm">
            <span className="text-2xl font-bold">
              {subscription.monthly_generations_used}
            </span>
            <span className="text-xs text-muted-foreground">
              上限 {subscription.monthly_generations_limit} 回
            </span>
          </div>
          <Progress value={genPercent} className="h-2" />
          <div className="flex items-center justify-between text-[11px] text-muted-foreground pt-1">
            <span>使用率: {genPercent}%</span>
            {genPercent >= 80 && (
              <span className="text-destructive font-medium">残りわずか</span>
            )}
          </div>
        </CardContent>
      </Card>

      {/* プロジェクト数メーター */}
      <Card>
        <CardHeader className="pb-2">
          <div className="flex items-center justify-between">
            <CardTitle className="text-sm font-medium flex items-center gap-1.5">
              <Boxes className="h-4 w-4 text-primary" />
              作成プロジェクト数
            </CardTitle>
            <span className="text-xs text-muted-foreground">
              {subscription.projects_used} / {subscription.projects_limit}
            </span>
          </div>
          <CardDescription className="text-xs">
            同時稼働中のアクティブプロジェクト
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-2">
          <div className="flex items-baseline justify-between text-sm">
            <span className="text-2xl font-bold">
              {subscription.projects_used}
            </span>
            <span className="text-xs text-muted-foreground">
              最大 {subscription.projects_limit} 件
            </span>
          </div>
          <Progress value={projPercent} className="h-2" />
          <div className="flex items-center justify-between text-[11px] text-muted-foreground pt-1">
            <span>残り枠: {subscription.projects_limit - subscription.projects_used} 件</span>
            <Button variant="ghost" size="sm" className="h-6 text-[11px] px-1 gap-1">
              枠を追加 <ArrowUpRight className="h-3 w-3" />
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
