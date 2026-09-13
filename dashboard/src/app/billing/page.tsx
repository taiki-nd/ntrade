"use client"

import * as React from "react"
import { DashboardLayout } from "@/components/layouts/dashboard-layout"
import { UsageMeter } from "@/components/billing/usage-meter"
import { useSubscription, useProjects, useGenerationLogs } from "@/lib/api"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { CreditCard, ExternalLink, Check, Sparkles, ShieldCheck, Loader2 } from "lucide-react"

export default function BillingPage() {
  const { subscription, loading: subLoading } = useSubscription()
  const { projects } = useProjects()
  const { logs } = useGenerationLogs()

  const activeSub = subscription
    ? {
        id: subscription.id,
        plan: subscription.plan,
        status: subscription.status,
        current_period_end: subscription.current_period_end || new Date().toISOString(),
        cancel_at_period_end: subscription.cancel_at_period_end,
        monthly_generations_used: logs.length,
        monthly_generations_limit: 300,
        projects_used: projects.length,
        projects_limit: 10,
      }
    : {
        id: "sub_free",
        plan: "free" as const,
        status: "active" as const,
        current_period_end: new Date().toISOString(),
        cancel_at_period_end: false,
        monthly_generations_used: logs.length,
        monthly_generations_limit: 50,
        projects_used: projects.length,
        projects_limit: 3,
      }

  return (
    <DashboardLayout title="請求・サブスクリプション">
      <div className="space-y-6">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">請求・プラン管理</h2>
          <p className="text-muted-foreground text-sm">
            ご契約プラン、月間リソース使用量、および支払い情報の管理。
          </p>
        </div>

        {/* 使用状況メーター */}
        <UsageMeter subscription={activeSub} />

        {/* 現在の契約プラン詳細 & Stripe Portal */}
        <div className="grid gap-6 md:grid-cols-2">
          <Card>
            <CardHeader className="pb-3">
              <div className="flex items-center justify-between">
                <Badge variant="outline" className="bg-accent text-accent-foreground uppercase font-bold text-xs">
                  {activeSub.plan} プラン
                </Badge>
                <span className="text-xs text-muted-foreground">月額契約</span>
              </div>
              <CardTitle className="text-2xl font-bold mt-2">
                {activeSub.plan === "enterprise" ? "要相談" : activeSub.plan === "team" ? "¥49,800" : activeSub.plan === "pro" ? "¥19,800" : "¥0"}
                <span className="text-sm font-normal text-muted-foreground"> / 月</span>
              </CardTitle>
              <CardDescription className="text-xs">
                次回更新日: {new Date(activeSub.current_period_end).toLocaleDateString("ja-JP")}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2 text-xs text-muted-foreground">
                <div className="flex items-center gap-2">
                  <Check className="h-3.5 w-3.5 text-primary" />
                  <span>最大 {activeSub.projects_limit} プロジェクト同時管理 (現在: {activeSub.projects_used} 件)</span>
                </div>
                <div className="flex items-center gap-2">
                  <Check className="h-3.5 w-3.5 text-primary" />
                  <span>月 {activeSub.monthly_generations_limit} 回の自律コード生成 ＆ AI 壁打ち (現在: {activeSub.monthly_generations_used} 回)</span>
                </div>
                <div className="flex items-center gap-2">
                  <Check className="h-3.5 w-3.5 text-primary" />
                  <span>受託クライアント無制限招待 ＆ 権限分離</span>
                </div>
              </div>

              <div className="pt-2 border-t border-border">
                <Button variant="outline" size="sm" className="w-full gap-1.5 text-xs" onClick={() => alert("Stripe ポータルへ連携します")}>
                  <CreditCard className="h-3.5 w-3.5" />
                  お支払い方法・請求書の管理 (Stripe Portal)
                  <ExternalLink className="h-3 w-3 ml-auto opacity-60" />
                </Button>
              </div>
            </CardContent>
          </Card>

          {/* 追加クレジット / アドオン購入 */}
          <Card>
            <CardHeader className="pb-3">
              <div className="flex items-center gap-2">
                <Sparkles className="h-4 w-4 text-primary" />
                <CardTitle className="text-base font-semibold">生成枠の追加（アドオン）</CardTitle>
              </div>
              <CardDescription className="text-xs">
                月間上限に達した場合でも、即座に追加コード生成枠を購入可能です。
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="p-3 bg-muted/40 rounded-lg border border-border flex items-center justify-between">
                <div>
                  <p className="font-semibold text-xs">生成枠 +100回 パック</p>
                  <p className="text-[11px] text-muted-foreground">有効期限: 当月末まで</p>
                </div>
                <Button size="sm" variant="secondary" className="h-7 text-xs px-3">
                  ¥9,800 で購入
                </Button>
              </div>

              <div className="p-3 bg-muted/40 rounded-lg border border-border flex items-center justify-between">
                <div>
                  <p className="font-semibold text-xs">生成枠 +300回 パック</p>
                  <p className="text-[11px] text-muted-foreground">有効期限: 当月末まで (20%お得)</p>
                </div>
                <Button size="sm" variant="secondary" className="h-7 text-xs px-3">
                  ¥24,800 で購入
                </Button>
              </div>

              <div className="flex items-center gap-2 pt-2 text-[11px] text-muted-foreground">
                <ShieldCheck className="h-3.5 w-3.5 text-emerald-500 shrink-0" />
                <span>未使用の追加枠は翌月には繰り越されません。</span>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </DashboardLayout>
  )
}
