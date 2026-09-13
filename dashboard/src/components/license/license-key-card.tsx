"use client"

import * as React from "react"
import { LicenseKey } from "@/types/schema"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { KeyRound, Copy, Check, Eye, EyeOff, ShieldCheck, RefreshCw } from "lucide-react"

interface LicenseKeyCardProps {
  licenseKey: LicenseKey
}

export function LicenseKeyCard({ licenseKey }: LicenseKeyCardProps) {
  const [copied, setCopied] = React.useState(false)
  const [showFull, setShowFull] = React.useState(false)

  const handleCopy = () => {
    navigator.clipboard.writeText(licenseKey.key_masked)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <Card>
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <KeyRound className="h-5 w-5 text-primary" />
            <CardTitle className="text-base font-bold">
              Enterprise ライセンスキー
            </CardTitle>
          </div>
          <Badge variant="outline" className="bg-accent text-accent-foreground">
            <ShieldCheck className="mr-1 h-3 w-3" />
            有効
          </Badge>
        </div>
        <CardDescription className="text-xs">
          セルフホスト版 bee8 エンジン（Docker / k8s）の認証およびオフライン暗号検証用キー
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4 text-sm">
        {/* キー表示ボックス */}
        <div className="p-3 bg-muted/50 rounded-lg border border-border flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div className="font-mono text-xs text-foreground truncate max-w-full sm:max-w-[70%]">
            {showFull ? licenseKey.key_masked : `${licenseKey.key_prefix}****************`}
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <Button
              variant="outline"
              size="sm"
              className="h-8 text-xs gap-1"
              onClick={() => setShowFull(!showFull)}
            >
              {showFull ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
              {showFull ? "隠す" : "表示"}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              className="h-8 text-xs gap-1"
              onClick={handleCopy}
            >
              {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
              {copied ? "コピー済み" : "コピー"}
            </Button>
          </div>
        </div>

        {/* メタ情報 */}
        <div className="grid grid-cols-2 gap-3 text-xs pt-1">
          <div>
            <span className="text-muted-foreground block">有効期限</span>
            <span className="font-medium text-foreground">
              {new Date(licenseKey.expires_at).toLocaleDateString("ja-JP")}
            </span>
          </div>
          <div>
            <span className="text-muted-foreground block">最終チェックイン確認</span>
            <span className="font-medium text-foreground">
              {licenseKey.last_verified_at
                ? new Date(licenseKey.last_verified_at).toLocaleString("ja-JP")
                : "未接続"}
            </span>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
