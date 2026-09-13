"use client"

import * as React from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"
import { DashboardLayout } from "@/components/layouts/dashboard-layout"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Button, buttonVariants } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Badge } from "@/components/ui/badge"
import { ArrowLeft, Sparkles, Bot, ArrowRight, Check, Database, Loader2 } from "lucide-react"
import { sendSetupChat } from "@/lib/api"
import { createProject } from "@/generated/api/projectApi"

export default function NewProjectPage() {
  const router = useRouter()
  const [step, setStep] = React.useState<1 | 2 | 3>(1)
  const [name, setName] = React.useState("")
  const [dbType, setDbType] = React.useState<"postgres" | "mysql" | "sqlite">("postgres")
  const [prompt, setPrompt] = React.useState("")
  const [suggestedDomains, setSuggestedDomains] = React.useState<string[]>(["account", "deal", "quote", "invoice"])
  const [isAnalyzing, setIsAnalyzing] = React.useState(false)
  const [isGenerating, setIsGenerating] = React.useState(false)

  const handleNextToStep3 = async () => {
    setIsAnalyzing(true)
    try {
      const res = await sendSetupChat(prompt, [])
      if (res.data?.suggested_domains && res.data.suggested_domains.length > 0) {
        setSuggestedDomains(res.data.suggested_domains)
      }
    } finally {
      setIsAnalyzing(false)
      setStep(3)
    }
  }

  const handleCreate = async () => {
    setIsGenerating(true)
    try {
      const slug = name.toLowerCase().replace(/[^a-z0-9]/g, "-").replace(/-+/g, "-") || "project"
      const res = await createProject({
        name,
        slug,
        database_type: dbType,
        status: "active",
        setup_prompt: prompt,
        identity_id: "",
        organization_id: "",
        preview_url: "",
        repo_url: "",
      })
      if (res.data?.id) {
        router.push(`/projects/${res.data.id}`)
        return
      }
    } catch (err) {
      console.warn("API create project fallback:", err)
    } finally {
      setIsGenerating(false)
    }
    // Fallback to mock project
    router.push("/projects/prj_01H1A2B3C4D5E6F7G8H9J0K1L2")
  }

  return (
    <DashboardLayout title="新規プロジェクト立ち上げ">
      <div className="max-w-3xl mx-auto space-y-6">
        <div className="flex items-center gap-3">
          <Link
            href="/projects"
            className={cn(buttonVariants({ variant: "ghost", size: "sm" }), "gap-1 px-2 h-8")}
          >
            <ArrowLeft className="h-4 w-4" />
            プロジェクト一覧へ戻る
          </Link>
        </div>

        <div>
          <div className="flex items-center gap-2 mb-1">
            <Badge variant="secondary" className="gap-1 text-xs">
              <Sparkles className="h-3 w-3 text-primary" />
              AI Setup Assistant
            </Badge>
            <span className="text-xs text-muted-foreground">ステップ {step} / 3</span>
          </div>
          <h2 className="text-2xl font-bold tracking-tight">
            プロジェクト新規作成 ＆ AI設計
          </h2>
          <p className="text-muted-foreground text-sm">
            業務課題を自然言語で入力すると、AIが初期ドメインモデル・画面構成・Goバックエンドを自動ドラフトします。
          </p>
        </div>

        {/* ステップ1: 基本情報 */}
        {step === 1 && (
          <Card>
            <CardHeader>
              <CardTitle className="text-base">1. プロジェクト基本情報</CardTitle>
              <CardDescription className="text-xs">
                プロジェクト名称と利用するリレーショナルデータベースを選択してください。
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-foreground">
                  プロジェクト名
                </label>
                <Input
                  placeholder="例: 社内営業案件・見積管理システム"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-medium text-foreground">
                  データベース種別
                </label>
                <div className="grid grid-cols-3 gap-3">
                  {(["postgres", "mysql", "sqlite"] as const).map((type) => (
                    <button
                      key={type}
                      type="button"
                      onClick={() => setDbType(type)}
                      className={`p-3 rounded-lg border text-left flex flex-col justify-between transition-colors ${
                        dbType === type
                          ? "border-primary bg-primary/5 text-foreground"
                          : "border-border hover:bg-muted/40 text-muted-foreground"
                      }`}
                    >
                      <Database className="h-4 w-4 mb-2 text-primary" />
                      <span className="font-semibold text-xs capitalize text-foreground">
                        {type}
                      </span>
                    </button>
                  ))}
                </div>
              </div>

              <div className="pt-2 flex justify-end">
                <Button
                  onClick={() => setStep(2)}
                  disabled={!name.trim()}
                  className="gap-1.5"
                >
                  次へ: AI要件ヒアリング
                  <ArrowRight className="h-4 w-4" />
                </Button>
              </div>
            </CardContent>
          </Card>
        )}

        {/* ステップ2: AI要件ヒアリング */}
        {step === 2 && (
          <Card>
            <CardHeader>
              <CardTitle className="text-base">2. AI 要件ヒアリング（Setup Chat）</CardTitle>
              <CardDescription className="text-xs">
                「誰のどんな課題を解決するか」「必要な機能やロール」を自由に入力してください。
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="p-3.5 bg-muted/40 border border-border rounded-lg text-xs space-y-2">
                <div className="flex items-center gap-1.5 font-medium text-foreground">
                  <Bot className="h-4 w-4 text-primary" />
                  AI アシスタントからのヒント
                </div>
                <p className="text-muted-foreground leading-relaxed">
                  例: 「受託クライアント向けの案件見積り・請求書発行システム。営業担当者が案件を作成し、管理者が承認後にクライアントへPDFメール送信。クライアントはログインして請求明細を閲覧可能にしたい。」
                </p>
              </div>

              <Textarea
                placeholder="解決したい業務課題や機能要件を入力..."
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                rows={5}
                className="text-sm resize-none"
              />

              <div className="flex items-center justify-between pt-2">
                <Button variant="outline" size="sm" onClick={() => setStep(1)}>
                  戻る
                </Button>
                <Button
                  size="sm"
                  onClick={handleNextToStep3}
                  disabled={!prompt.trim() || isAnalyzing}
                  className="gap-1.5"
                >
                  {isAnalyzing ? (
                    <>
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      要件分析中...
                    </>
                  ) : (
                    <>
                      提案サマリーをプレビュー
                      <ArrowRight className="h-4 w-4" />
                    </>
                  )}
                </Button>
              </div>
            </CardContent>
          </Card>
        )}

        {/* ステップ3: 提案サマリー確認 & 初回生成 */}
        {step === 3 && (
          <Card>
            <CardHeader>
              <CardTitle className="text-base">3. AI 生成計画の確認</CardTitle>
              <CardDescription className="text-xs">
                以下のスキーマ定義とフルスタック構成で自動生成を開始します。
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4 text-sm">
              <div className="space-y-2 p-3 bg-muted/30 border border-border rounded-lg text-xs">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">プロジェクト名:</span>
                  <span className="font-semibold text-foreground">{name}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">データベース:</span>
                  <span className="font-mono capitalize text-foreground">{dbType}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">自動導出ドメイン:</span>
                  <span className="font-mono text-foreground">
                    {suggestedDomains.join(", ")} ({suggestedDomains.length} ドメイン)
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">bee8 lint 保証:</span>
                  <span className="text-foreground font-semibold">
                    txcheck / querycheck / authcheck: 有効
                  </span>
                </div>
              </div>

              <div className="flex items-center justify-between pt-2">
                <Button variant="outline" size="sm" onClick={() => setStep(2)}>
                  要件を修正
                </Button>
                <Button
                  size="sm"
                  onClick={handleCreate}
                  disabled={isGenerating}
                  className="gap-1.5"
                >
                  {isGenerating ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin" />
                      コード生成 & プレビュー起動中...
                    </>
                  ) : (
                    <>
                      <Sparkles className="h-4 w-4" />
                      作成して初回生成を実行
                    </>
                  )}
                </Button>
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </DashboardLayout>
  )
}
