"use client"

import * as React from "react"
import Link from "next/link"
import {
  Zap,
  ShieldCheck,
  Code2,
  Cpu,
  Palette,
  CheckCircle2,
  Layers,
  Sparkles,
} from "lucide-react"

import { MarketingLayout } from "@/components/layouts/marketing-layout"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { buttonVariants } from "@/components/ui/button"
import { cn } from "@/lib/utils"

export default function FeaturesPage() {
  return (
    <MarketingLayout>
      <div className="container mx-auto max-w-5xl px-4 py-8 sm:py-12 space-y-16">
        {/* Page Header */}
        <div className="text-center space-y-4 max-w-3xl mx-auto">
          <div className="inline-flex items-center gap-2 rounded-full border bg-muted/50 px-3 py-1 text-xs font-medium">
            <Zap className="h-3.5 w-3.5 text-primary" />
            <span>Architecture & Principles</span>
          </div>
          <h1 className="text-3xl font-extrabold tracking-tight sm:text-5xl">
            Features & Architecture
          </h1>
          <p className="text-base sm:text-lg text-muted-foreground">
            AI時代のフロントエンド開発を最速・最小トークンで実現する Next.js + shadcn/ui 基盤。
            車輪の再発明を防止し、常に最高品質のデザインを担保します。
          </p>
        </div>

        {/* 3 Core Pillars */}
        <div className="grid gap-6 sm:grid-cols-3">
          <Card className="flex flex-col justify-between">
            <CardHeader>
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10 text-primary mb-3">
                <Zap className="h-5 w-5" />
              </div>
              <CardTitle className="text-xl">トークン消費 70% 削減</CardTitle>
              <CardDescription className="text-sm leading-relaxed mt-2">
                生のCSSやTailwindの長大なクラス羅列を毎回AIに出力させず、既存の整備済みパーツをインポートさせるだけでUIを構築。AIの応答速度が向上し、APIコストを劇的に削減します。
              </CardDescription>
            </CardHeader>
            <CardContent className="pt-0">
              <ul className="space-y-2 text-xs text-muted-foreground">
                <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-3.5 w-3.5 text-primary shrink-0" />
                  <span>31個のUIパーツをあらかじめ内蔵</span>
                </li>
                <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-3.5 w-3.5 text-primary shrink-0" />
                  <span>重複コード生成の自動防止</span>
                </li>
              </ul>
            </CardContent>
          </Card>

          <Card className="flex flex-col justify-between">
            <CardHeader>
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10 text-primary mb-3">
                <ShieldCheck className="h-5 w-5" />
              </div>
              <CardTitle className="text-xl">「AI臭さ」ゼロの美意識</CardTitle>
              <CardDescription className="text-sm leading-relaxed mt-2">
                派手すぎるグラデーションや安っぽい装飾を禁止し、Linear / Stripe / Apple 風の洗練されたミニマリズムを徹底。一目で「プロのプロダクト」と分かる質感を維持します。
              </CardDescription>
            </CardHeader>
            <CardContent className="pt-0">
              <ul className="space-y-2 text-xs text-muted-foreground">
                <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-3.5 w-3.5 text-primary shrink-0" />
                  <span>セマンティックカラートークン完全準拠</span>
                </li>
                <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-3.5 w-3.5 text-primary shrink-0" />
                  <span>ダークモード & 6色のブランドカラー対応</span>
                </li>
              </ul>
            </CardContent>
          </Card>

          <Card className="flex flex-col justify-between">
            <CardHeader>
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10 text-primary mb-3">
                <Code2 className="h-5 w-5" />
              </div>
              <CardTitle className="text-xl">AGENTS.md 同梱</CardTitle>
              <CardDescription className="text-sm leading-relaxed mt-2">
                Claude Code や Antigravity、Cursor などのAIエージェントがプロジェクトの掟を読み込めるよう、設計原則とルールをドキュメント化して同梱しています。
              </CardDescription>
            </CardHeader>
            <CardContent className="pt-0">
              <ul className="space-y-2 text-xs text-muted-foreground">
                <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-3.5 w-3.5 text-primary shrink-0" />
                  <span>AIへのコンテキスト自動注入</span>
                </li>
                <li className="flex items-center gap-2">
                  <CheckCircle2 className="h-3.5 w-3.5 text-primary shrink-0" />
                  <span>掟違反の自動抑制</span>
                </li>
              </ul>
            </CardContent>
          </Card>
        </div>

        {/* Detailed Tech Highlights */}
        <div className="space-y-6">
          <div className="space-y-2">
            <h2 className="text-2xl font-bold tracking-tight">Technical Stack & Capabilities</h2>
            <p className="text-muted-foreground text-sm">
              最新のモダンフロントエンドスタックを組み合わせた堅牢なアーキテクチャ。
            </p>
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <Card>
              <CardHeader>
                <div className="flex items-center gap-3">
                  <Cpu className="h-5 w-5 text-primary" />
                  <CardTitle className="text-base">Base UI (MUI) 採用</CardTitle>
                </div>
                <CardDescription className="text-xs sm:text-sm pt-2">
                  Radix UI の代替として最新の <code>@base-ui-components/react</code> を採用。
                  TypeScript Strict Mode における型エラーを根絶し、React 19 に完全対応したアクセシブルな基盤です。
                </CardDescription>
              </CardHeader>
            </Card>

            <Card>
              <CardHeader>
                <div className="flex items-center gap-3">
                  <Palette className="h-5 w-5 text-primary" />
                  <CardTitle className="text-base">CSS変数ベースのカラーシステム</CardTitle>
                </div>
                <CardDescription className="text-xs sm:text-sm pt-2">
                  <code>zinc</code>, <code>slate</code>, <code>rose</code>, <code>blue</code>, <code>green</code>, <code>orange</code> のカラーテーマをワンクリックで切り替え。
                  ハードコードされた色指定を排除し、全体の整合性を守ります。
                </CardDescription>
              </CardHeader>
            </Card>
          </div>
        </div>

        {/* CTA Section */}
        <div className="rounded-xl border bg-card p-8 text-center space-y-4">
          <h3 className="text-xl font-bold">実際にコンポーネントやプロンプトを試す</h3>
          <p className="text-muted-foreground text-sm max-w-lg mx-auto">
            整備されたUIコンポーネントの一覧を確認するか、AIへの指示出し用プロンプト集をチェックしましょう。
          </p>
          <div className="flex flex-wrap items-center justify-center gap-3 pt-2">
            <Link
              href="/components"
              className={cn(buttonVariants({ size: "sm" }), "gap-1.5")}
            >
              <Layers className="h-4 w-4" />
              View Components (31)
            </Link>
            <Link
              href="/prompts"
              className={cn(buttonVariants({ variant: "outline", size: "sm" }), "gap-1.5")}
            >
              <Sparkles className="h-4 w-4" />
              Explore Prompt Guide
            </Link>
          </div>
        </div>
      </div>
    </MarketingLayout>
  )
}
