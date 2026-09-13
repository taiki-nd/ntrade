"use client"

import * as React from "react"
import Link from "next/link"
import { toast } from "sonner"
import { Sparkles, Check, Copy, Code2, ArrowRight, Layers, Lightbulb } from "lucide-react"

import { MarketingLayout } from "@/components/layouts/marketing-layout"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Button, buttonVariants } from "@/components/ui/button"
import { cn } from "@/lib/utils"

export default function PromptsPage() {
  const [copiedKey, setCopiedKey] = React.useState<string | null>(null)

  const copyPrompt = (text: string, key: string) => {
    navigator.clipboard.writeText(text)
    setCopiedKey(key)
    toast.success("プロンプトをクリップボードにコピーしました！")
    setTimeout(() => setCopiedKey(null), 2000)
  }

  const prompts = [
    {
      id: "standard",
      label: "基本UIパーツ指定",
      description: "新しい管理画面やテーブル画面を作成する際の基本プロンプトです。",
      code: `「@/components/layouts/dashboard-layout.tsx を使い、
ユーザー一覧画面を実装してください。
UIパーツはすべて @/components/ui/ (Table, Badge, Button, Input, Dialog) を利用し、
独自CSSや余分なスタイルは書かないでください。」`,
    },
    {
      id: "color",
      label: "色合い・トーン指定",
      description: "カラーパレットや特定のトーンを指定して画面を作る際のプロンプトです。",
      code: `「@/components/layouts/dashboard-layout.tsx を使い、
請求・トランザクション管理画面を実装してください。
トーンは『Linear風のダークOLED背景に、Violetのアクセントカラー』でお願いします。
UIパーツ側への色クラス直打ちは禁止し、bg-primaryやbg-background等のセマンティックトークンのみで記述してください。」`,
    },
    {
      id: "component",
      label: "新コンポーネント追加",
      description: "新しいコンポーネントを設計原則に沿って作成させるプロンプトです。",
      code: `「Base UI と Tailwind CSS を使用して、再利用可能な『StatsCard』コンポーネントを
@/components/ui/stats-card.tsx に作成してください。
Lucide アイコン、タイトル、数値、前月比増減バッジを受け取れるようにし、
ハードコードされた色（bg-blue-500など）は避け、セマンティックトークン（bg-muted, text-primary等）を使用してください。」`,
    },
    {
      id: "refactor",
      label: "AI臭さの排除・リファクタ",
      description: "既存のコードの冗長なCSSを削ぎ落とし、クリーンに整えるプロンプトです。",
      code: `「この画面のスタイルをリファクタリングしてください。
不要なグラデーションや過剰なパディング・マージンを削除し、
shadcn/ui の標準クラスと @/components/ui/ の既存パーツに置き換えて、
ミニマルで洗練されたデザインに統一してください。」`,
    },
  ]

  return (
    <MarketingLayout>
      <div className="container mx-auto max-w-5xl px-4 py-8 sm:py-12 space-y-12">
        {/* Page Header */}
        <div className="text-center space-y-4 max-w-3xl mx-auto">
          <div className="inline-flex items-center gap-2 rounded-full border bg-muted/50 px-3 py-1 text-xs font-medium">
            <Code2 className="h-3.5 w-3.5 text-primary" />
            <span>AI Prompt Templates</span>
          </div>
          <h1 className="text-3xl font-extrabold tracking-tight sm:text-5xl">
            AI Prompt Guide
          </h1>
          <p className="text-base sm:text-lg text-muted-foreground">
            Claude Code や Antigravity、Cursor にそのままコピペして使える高品質な指示出しテンプレート。
            AIのトークン消費を最小化し、破綻のないUIを出力させます。
          </p>
        </div>

        {/* Prompt Card with Tabs */}
        <Card className="border-primary/20 bg-card">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-xl">
              <Sparkles className="h-5 w-5 text-primary" />
              コピペ用プロンプト集
            </CardTitle>
            <CardDescription>
              タブを切り替えて目的のプロンプトをワンクリックでコピーできます。
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Tabs defaultValue="standard" className="w-full">
              <TabsList className="mb-4 grid w-full grid-cols-2 sm:grid-cols-4">
                {prompts.map((p) => (
                  <TabsTrigger key={p.id} value={p.id} className="text-xs sm:text-sm">
                    {p.label}
                  </TabsTrigger>
                ))}
              </TabsList>

              {prompts.map((p) => (
                <TabsContent key={p.id} value={p.id} className="space-y-3">
                  <p className="text-xs sm:text-sm text-muted-foreground">{p.description}</p>
                  <div className="relative rounded-lg bg-muted p-4 font-mono text-sm leading-relaxed border">
                    <pre className="overflow-x-auto whitespace-pre-wrap text-xs sm:text-sm pr-20">
                      {p.code}
                    </pre>
                    <Button
                      size="sm"
                      variant="outline"
                      className="absolute top-3 right-3 gap-1.5 text-xs bg-background/80 backdrop-blur-xs cursor-pointer"
                      onClick={() => copyPrompt(p.code, p.id)}
                    >
                      {copiedKey === p.id ? (
                        <>
                          <Check className="h-3.5 w-3.5 text-green-500" />
                          <span>Copied!</span>
                        </>
                      ) : (
                        <>
                          <Copy className="h-3.5 w-3.5" />
                          <span>Copy</span>
                        </>
                      )}
                    </Button>
                  </div>
                </TabsContent>
              ))}
            </Tabs>
          </CardContent>
        </Card>

        {/* Best Practice Tips */}
        <div className="grid gap-6 sm:grid-cols-2">
          <Card>
            <CardHeader>
              <div className="flex items-center gap-2.5">
                <Lightbulb className="h-5 w-5 text-amber-500" />
                <CardTitle className="text-base">掟1: 既存コンポーネントを必ず指定する</CardTitle>
              </div>
              <CardDescription className="text-xs sm:text-sm pt-2">
                AIに「ボタンを置いて」と頼むと、<code>&lt;button className=&quot;...&quot;&gt;</code> と生で出力してしまいトークンを浪費します。
                <code>@/components/ui/button</code> の <code>Button</code> を使うよう明示的に指示しましょう。
              </CardDescription>
            </CardHeader>
          </Card>

          <Card>
            <CardHeader>
              <div className="flex items-center gap-2.5">
                <Lightbulb className="h-5 w-5 text-amber-500" />
                <CardTitle className="text-base">掟2: セマンティックトークンを使う</CardTitle>
              </div>
              <CardDescription className="text-xs sm:text-sm pt-2">
                <code>bg-blue-600</code> や <code>text-gray-400</code> ではなく、
                <code>bg-primary</code>, <code>text-muted-foreground</code>, <code>bg-background</code> などのセマンティッククラスを使わせることで、ダークモードやテーマ変更に自動追従します。
              </CardDescription>
            </CardHeader>
          </Card>
        </div>

        {/* Bottom CTA */}
        <div className="text-center pt-4">
          <Link
            href="/components"
            className={cn(buttonVariants({ size: "default" }), "gap-2")}
          >
            <Layers className="h-4 w-4" />
            コンポーネント一覧（31パーツ）を見に行く
            <ArrowRight className="h-4 w-4" />
          </Link>
        </div>
      </div>
    </MarketingLayout>
  )
}
