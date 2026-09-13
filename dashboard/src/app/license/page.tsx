"use client"

import * as React from "react"
import { DashboardLayout } from "@/components/layouts/dashboard-layout"
import { LicenseKeyCard } from "@/components/license/license-key-card"
import { useLicenseKey } from "@/lib/api"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Terminal, RefreshCw, AlertTriangle, ShieldCheck, KeyRound } from "lucide-react"

export default function LicensePage() {
  const { licenseKey, loading, refetch } = useLicenseKey()

  const defaultKey = licenseKey
    ? {
        id: licenseKey.id,
        subscription_id: "sub_live",
        key_prefix: (licenseKey as any).key_prefix || "bee8_ent_live",
        key_hash: "********************************",
        key_masked: "bee8_ent_live_••••••••",
        status: "active" as const,
        last_verified_at: (licenseKey as any).last_verified_at,
        expires_at: licenseKey.expires_at,
        created_at: licenseKey.created_at,
      }
    : null

  return (
    <DashboardLayout title="Enterprise ライセンス管理">
      <div className="space-y-6">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">ライセンス管理</h2>
          <p className="text-muted-foreground text-sm">
            Enterprise 契約向けのセルフホスト（オンプレミス / VPC）認証キーおよび接続ステータス。
          </p>
        </div>

        {/* ライセンスキーカード */}
        {defaultKey ? (
          <LicenseKeyCard licenseKey={defaultKey} />
        ) : (
          <Card className="p-8 text-center bg-muted/20 border-dashed">
            <KeyRound className="mx-auto h-8 w-8 text-muted-foreground mb-3" />
            <h3 className="text-base font-semibold mb-1">アクティブなライセンスキーがありません</h3>
            <p className="text-xs text-muted-foreground mb-4">
              Enterprise プランをご契約いただくと、セルフホスト用オンプレミスライセンスキーが発行されます。
            </p>
            <Button size="sm" onClick={() => refetch()} variant="outline">
              再確認する
            </Button>
          </Card>
        )}

        {/* セルフホスト設定手順ガイド */}
        <div className="grid gap-6 md:grid-cols-2">
          <Card>
            <CardHeader className="pb-3">
              <div className="flex items-center gap-2">
                <Terminal className="h-4 w-4 text-primary" />
                <CardTitle className="text-base font-semibold">
                  環境変数での設定方法
                </CardTitle>
              </div>
              <CardDescription className="text-xs">
                セルフホスト bee8 実行コンテナへのキー適用
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="p-3 bg-muted font-mono text-xs rounded-lg overflow-x-auto text-muted-foreground space-y-1">
                <div># docker-compose.yml または k8s Secret</div>
                <div>BEE8_LICENSE_KEY={defaultKey?.key_prefix || "bee8_ent_..."}...</div>
                <div>BEE8_AIRGAP_MODE=false</div>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="pb-3">
              <div className="flex items-center gap-2">
                <ShieldCheck className="h-4 w-4 text-primary" />
                <CardTitle className="text-base font-semibold">
                  キーの再発行 ＆ 失効
                </CardTitle>
              </div>
              <CardDescription className="text-xs">
                漏洩時またはサーバー移行時の再発行手続き
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3 text-xs">
              <p className="text-muted-foreground">
                ライセンスキーを再発行すると、旧キーは 24 時間後に自動的に失効します。
              </p>
              <Button variant="outline" size="sm" className="gap-1.5 text-xs">
                <RefreshCw className="h-3.5 w-3.5" />
                ライセンスキーを再生成
              </Button>
            </CardContent>
          </Card>
        </div>
      </div>
    </DashboardLayout>
  )
}
