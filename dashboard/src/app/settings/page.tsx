"use client";

import * as React from "react";
import Link from "next/link";
import { ArrowLeft, UserCheck } from "lucide-react";

import { DashboardLayout } from "@/components/layouts/dashboard-layout";
import { buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { PasskeyManagement } from "@/components/auth/passkey-management";
import { useAuth } from "@/contexts/auth-context";
import { cn } from "@/lib/utils";

export default function SettingsPage() {
  const { user, isLoading } = useAuth();

  return (
    <DashboardLayout title="Settings">
      <div className="space-y-6 max-w-4xl">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div>
            <h2 className="text-2xl font-bold tracking-tight">Account & Security</h2>
            <p className="text-muted-foreground text-sm">
              アカウント情報およびパスキー（生体認証 / セキュリティキー）の設定を管理します。
            </p>
          </div>
          <Link
            href="/dashboard"
            className={cn(buttonVariants({ variant: "outline", size: "sm" }), "gap-1.5 self-start sm:self-auto")}
          >
            <ArrowLeft className="h-4 w-4" />
            Back to Dashboard
          </Link>
        </div>

        {/* User Info Card */}
        <Card>
          <CardHeader>
            <div className="flex items-center gap-2">
              <UserCheck className="h-5 w-5 text-primary" />
              <CardTitle className="text-lg">ユーザー情報</CardTitle>
            </div>
            <CardDescription>
              現在ログイン中のセッション情報です。
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {isLoading ? (
              <p className="text-sm text-muted-foreground">読み込み中...</p>
            ) : user ? (
              <div className="grid gap-3 sm:grid-cols-2 text-sm">
                <div>
                  <span className="text-xs font-semibold text-muted-foreground block">ユーザーID</span>
                  <span className="font-mono text-xs">{user.id}</span>
                </div>
                <div>
                  <span className="text-xs font-semibold text-muted-foreground block">メールアドレス</span>
                  <span>{user.email}</span>
                </div>
                <div>
                  <span className="text-xs font-semibold text-muted-foreground block">ロール (権限)</span>
                  <Badge variant="secondary" className="mt-1 capitalize">
                    {user.role || "user"}
                  </Badge>
                </div>
              </div>
            ) : (
              <div className="text-sm text-muted-foreground">
                <p>現在ログインしていません。</p>
                <Link
                  href="/login"
                  className={cn(buttonVariants({ size: "sm" }), "mt-3")}
                >
                  ログインページへ
                </Link>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Passkey Security Management */}
        <PasskeyManagement />
      </div>
    </DashboardLayout>
  );
}
