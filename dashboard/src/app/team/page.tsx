"use client"

import * as React from "react"
import { DashboardLayout } from "@/components/layouts/dashboard-layout"
import { useMemberships } from "@/lib/api"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { UserPlus, Shield, User, Users, RefreshCw } from "lucide-react"

export default function TeamPage() {
  const { memberships, loading, refetch } = useMemberships()

  // デフォルトでログイン中ユーザーのアカウントを表示
  const displayMembers = memberships.length > 0
    ? memberships
    : [
        {
          id: "mem_current",
          organization_id: "org_default",
          identity_id: "01M2AR9A8DXWC276J4JKTCNJTG",
          role: "owner" as const,
          created_at: new Date().toISOString(),
        },
      ]

  return (
    <DashboardLayout title="チーム・クライアント管理">
      <div className="space-y-6">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div>
            <h2 className="text-2xl font-bold tracking-tight">チーム ＆ クライアント管理</h2>
            <p className="text-muted-foreground text-sm">
              社内開発メンバーおよび受託先クライアントの招待・権限ロール管理。
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button size="sm" variant="outline" onClick={() => refetch()} disabled={loading} className="gap-1.5">
              <RefreshCw className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
              更新
            </Button>
            <Button size="sm" className="gap-1.5 self-start sm:self-auto" onClick={() => alert("招待ダイアログを開きます")}>
              <UserPlus className="h-4 w-4" />
              メンバーを招待
            </Button>
          </div>
        </div>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base font-semibold">
              参加メンバー ({displayMembers.length} 名)
            </CardTitle>
            <CardDescription className="text-xs">
              クライアントロールのユーザーは許可されたプロジェクトの課題起票とプレビュー確認のみ行えます。
            </CardDescription>
          </CardHeader>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>メンバー</TableHead>
                  <TableHead>権限ロール</TableHead>
                  <TableHead className="hidden sm:table-cell">参加日</TableHead>
                  <TableHead className="text-right">アクション</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {displayMembers.map((member) => (
                  <TableRow key={member.id}>
                    <TableCell>
                      <div className="flex items-center gap-3">
                        <Avatar className="h-8 w-8">
                          <AvatarFallback className="text-xs bg-muted">
                            U
                          </AvatarFallback>
                        </Avatar>
                        <div>
                          <div className="font-medium text-xs sm:text-sm">
                            testuser@bee8.dev (現在のユーザー)
                          </div>
                          <div className="text-muted-foreground text-[11px]">
                            ID: {member.identity_id}
                          </div>
                        </div>
                      </div>
                    </TableCell>
                    <TableCell>
                      <Badge
                        variant={member.role === "owner" ? "default" : "outline"}
                        className="text-xs gap-1"
                      >
                        {member.role === "owner" ? (
                          <Shield className="h-3 w-3" />
                        ) : (
                          <User className="h-3 w-3" />
                        )}
                        {member.role === "owner" ? "オーナー" : "メンバー"}
                      </Badge>
                    </TableCell>
                    <TableCell className="hidden sm:table-cell text-xs text-muted-foreground">
                      {new Date(member.created_at).toLocaleDateString("ja-JP")}
                    </TableCell>
                    <TableCell className="text-right">
                      <Button variant="ghost" size="sm" className="h-7 text-xs">
                        編集
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </div>
    </DashboardLayout>
  )
}
