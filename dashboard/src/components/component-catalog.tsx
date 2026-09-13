"use client"

import * as React from "react"
import { toast } from "sonner"
import {
  Search,
  Copy,
  Check,
  Sparkles,
  Layers,
  ShieldCheck,
  Sliders,
  Info,
  User,
  Settings,
  LogOut,
  PanelLeft,
  AlertCircle,
  TrendingUp,
  FileCode,
  Bot,
  Plus,
  Inbox,
  Filter,
} from "lucide-react"

// UI components
import { Button, buttonVariants } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
  SheetFooter,
  SheetClose,
} from "@/components/ui/sheet"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion"
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
  AvatarBadge,
  AvatarGroup,
  AvatarGroupCount,
} from "@/components/ui/avatar"
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb"
import { Separator } from "@/components/ui/separator"
import { Skeleton } from "@/components/ui/skeleton"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { ThemeToggle } from "@/components/theme-toggle"
import { ColorThemeToggle } from "@/components/color-theme-toggle"
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert"
import { Progress } from "@/components/ui/progress"
import { Slider } from "@/components/ui/slider"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import {
  Pagination,
  PaginationContent,
  PaginationItem,
  PaginationLink,
  PaginationNext,
  PaginationPrevious,
  PaginationEllipsis,
} from "@/components/ui/pagination"
import { cn } from "@/lib/utils"

export interface ComponentCatalogProps {
  onSelectLayout?: (layout: "showcase" | "dashboard" | "auth") => void
}

type CategoryType = "all" | "blocks" | "buttons" | "forms" | "overlays" | "data-display" | "feedback" | "layouts"

export function ComponentCatalog({ onSelectLayout }: ComponentCatalogProps) {
  const [copiedId, setCopiedId] = React.useState<string | null>(null)
  const [codeViewMap, setCodeViewMap] = React.useState<Record<string, boolean>>({})
  const [searchQuery, setSearchQuery] = React.useState("")
  const [selectedCategory, setSelectedCategory] = React.useState<CategoryType>("all")

  // Interactive Playground states
  const [progressVal, setProgressVal] = React.useState(68)
  const [sliderVal, setSliderVal] = React.useState([45])

  const copyToClipboard = (text: string, id: string, message = "コードをコピーしました") => {
    navigator.clipboard.writeText(text)
    setCopiedId(id)
    toast.success(message)
    setTimeout(() => {
      setCopiedId((curr) => (curr === id ? null : curr))
    }, 2000)
  }

  const toggleCodeView = (id: string) => {
    setCodeViewMap((prev) => ({ ...prev, [id]: !prev[id] }))
  }

  const copyAiCheatSheet = () => {
    const cheatSheet = `【AI向けUI実装ルール】
next-scaffoldに同梱済みの以下のコンポーネントのみを使用し、独自CSSや車輪の再発明を行わないでください。
UIコンポーネント:
- Button, Badge: "@/components/ui/button", "@/components/ui/badge"
- Input, Label, Textarea: "@/components/ui/input", "@/components/ui/label", "@/components/ui/textarea"
- Select: "@/components/ui/select"
- Switch, Checkbox, RadioGroup: "@/components/ui/switch", "@/components/ui/checkbox", "@/components/ui/radio-group"
- Slider, Progress: "@/components/ui/slider", "@/components/ui/progress"
- Dialog, Sheet, Popover: "@/components/ui/dialog", "@/components/ui/sheet", "@/components/ui/popover"
- DropdownMenu, Tooltip: "@/components/ui/dropdown-menu", "@/components/ui/tooltip"
- Card, Table, Accordion: "@/components/ui/card", "@/components/ui/table", "@/components/ui/accordion"
- Avatar, Breadcrumb, Separator, Skeleton, Alert, Pagination: "@/components/ui/*"
- Layouts: "@/components/layouts/dashboard-layout", "@/components/layouts/auth-layout", "@/components/layouts/marketing-layout"
- Toast: "sonner" (toast.success / toast.error)

【カラー＆トーン指定ルール】
UIパーツへの色クラス直打ち（bg-blue-600等）は厳禁。常にbg-primary, bg-background等のセマンティックトークンを使うこと。
色指定時は globals.css または data-color-theme="blue|emerald|violet|rose|orange"、data-base-tone="slate|stone|oled" で制御すること。`

    copyToClipboard(cheatSheet, "ai-cheatsheet", "AI用チートシート（全UI辞書）をコピーしました！")
  }

  const copyComponentAiPrompt = (name: string, importPath: string, usage: string) => {
    const prompt = `「${importPath} の ${name} コンポーネントを使い、${usage}を実装してください。独自CSSは禁止し、トークンを節約してシンプルかつ洗練されたモノトーン基調で出力してください。」`
    copyToClipboard(prompt, `ai-${name}`, `${name} のAI指示プロンプトをコピーしました！`)
  }

  const categories: { id: CategoryType; label: string; count: number }[] = [
    { id: "all", label: "すべて (31)", count: 31 },
    { id: "blocks", label: "UI Blocks (実戦パターン)", count: 4 },
    { id: "buttons", label: "Buttons & Badges", count: 2 },
    { id: "forms", label: "Forms & Inputs", count: 9 },
    { id: "overlays", label: "Overlays & Menus", count: 5 },
    { id: "data-display", label: "Data Display", count: 9 },
    { id: "feedback", label: "Feedback & Tabs", count: 2 },
    { id: "layouts", label: "Layouts & Theme", count: 4 },
  ]

  const renderCardHeader = (
    title: string,
    categoryBadge: string,
    importPath: string,
    importCode: string,
    jsxCode: string,
    aiUsageDesc: string,
    id: string
  ) => {
    const isShowingCode = !!codeViewMap[id]
    return (
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between gap-2">
          <div>
            <CardTitle className="text-base flex items-center gap-2">
              <span>{title}</span>
              <Badge variant="secondary" className="text-[10px]">{categoryBadge}</Badge>
            </CardTitle>
            <CardDescription className="text-xs font-mono text-muted-foreground mt-0.5">
              {importPath}
            </CardDescription>
          </div>
          <div className="flex items-center gap-1">
            <Button
              size="sm"
              variant={isShowingCode ? "secondary" : "ghost"}
              className="h-7 px-2 text-xs text-muted-foreground gap-1"
              onClick={() => toggleCodeView(id)}
              title="コード表示 / プレビュー切り替え"
            >
              <FileCode className="h-3.5 w-3.5" />
              <span className="hidden sm:inline">{isShowingCode ? "Preview" : "Code"}</span>
            </Button>
            <Button
              size="sm"
              variant="ghost"
              className="h-7 px-2 text-xs text-muted-foreground gap-1"
              onClick={() => copyComponentAiPrompt(title, importPath, aiUsageDesc)}
              title="AI指示プロンプトをコピー"
            >
              <Bot className="h-3.5 w-3.5 text-primary" />
              <span className="hidden sm:inline">AI指示</span>
            </Button>
            <Button
              size="sm"
              variant="ghost"
              className="h-7 px-2 text-xs text-muted-foreground"
              onClick={() => copyToClipboard(isShowingCode ? jsxCode : importCode, id)}
              title="コードをコピー"
            >
              {copiedId === id ? <Check className="h-3.5 w-3.5 text-green-500" /> : <Copy className="h-3.5 w-3.5" />}
            </Button>
          </div>
        </div>
      </CardHeader>
    )
  }

  return (
    <section id="components" className="scroll-mt-20 space-y-8">
      {/* Catalog Header */}
      <div className="flex flex-col gap-4 md:flex-row md:items-end md:justify-between border-b pb-6">
        <div>
          <div className="inline-flex items-center gap-1.5 rounded-full border bg-muted/60 px-3 py-0.5 text-xs font-medium text-muted-foreground mb-2">
            <Sparkles className="h-3 w-3 text-primary" />
            <span>31 Components + 4 UI Blocks Ready</span>
          </div>
          <h2 className="text-3xl font-extrabold tracking-tight sm:text-4xl">
            Component Catalog
          </h2>
          <p className="text-muted-foreground text-sm sm:text-base mt-1 max-w-2xl">
            全31コンポーネントと実戦UIブロックを完備。Preview / Code切り替え、ワンクリックコピー、AI専用指示プロンプトの生成に対応しています。
          </p>
        </div>

        {/* Action Buttons */}
        <div className="flex flex-wrap items-center gap-2">
          <ColorThemeToggle className="h-9 text-xs" />
          <Button
            size="sm"
            variant="outline"
            className="gap-1.5 text-xs h-9"
            onClick={copyAiCheatSheet}
          >
            <Bot className="h-4 w-4 text-primary" />
            <span>Copy AI Cheat Sheet</span>
          </Button>
          <div className="relative w-full sm:w-64">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              type="search"
              placeholder="パーツ・ブロックを検索..."
              className="pl-8 text-sm h-9"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
          </div>
        </div>
      </div>

      {/* Category Tabs */}
      <div className="flex flex-wrap gap-2 items-center">
        {categories.map((cat) => (
          <Button
            key={cat.id}
            variant={selectedCategory === cat.id ? "default" : "outline"}
            size="sm"
            onClick={() => setSelectedCategory(cat.id)}
            className="h-8 text-xs gap-1.5"
          >
            <span>{cat.label}</span>
          </Button>
        ))}
      </div>

      {/* Catalog Grid */}
      <div className="grid gap-6 md:grid-cols-2">
        {/* ========================================================= */}
        {/* 0. UI BLOCKS (実戦複合パターン) */}
        {/* ========================================================= */}
        {(selectedCategory === "all" || selectedCategory === "blocks") && (
          <>
            {/* Block 1: KPI / Metrics Block */}
            {("block metric stats kpi".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="md:col-span-2 border-primary/30 shadow-xs">
                {renderCardHeader(
                  "Metric & KPI Cards Block",
                  "UI Block",
                  "@/components/ui/card, badge",
                  `import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"\nimport { Badge } from "@/components/ui/badge"`,
                  `<div className="grid gap-4 sm:grid-cols-3">\n  <Card>...</Card>\n</div>`,
                  "ダッシュボード用の売上・アクティブユーザーKPIカード3列グリッド",
                  "block-kpi"
                )}
                <CardContent>
                  {codeViewMap["block-kpi"] ? (
                    <pre className="text-[11px] font-mono bg-muted p-3 rounded-md overflow-x-auto">
{`<div className="grid gap-4 sm:grid-cols-3">
  <Card>
    <CardHeader className="pb-2">
      <CardTitle className="text-xs font-medium text-muted-foreground">月間経常収益 (MRR)</CardTitle>
      <div className="text-2xl font-bold">¥4,820,000</div>
    </CardHeader>
    <CardContent>
      <Badge variant="outline" className="text-emerald-600 border-emerald-500/30 gap-1 text-[11px]">
        <TrendingUp className="h-3 w-3" /> +12.4% 前月比
      </Badge>
    </CardContent>
  </Card>
</div>`}
                    </pre>
                  ) : (
                    <div className="grid gap-4 sm:grid-cols-3">
                      <Card className="bg-background">
                        <CardHeader className="pb-2">
                          <CardTitle className="text-xs font-medium text-muted-foreground">月間経常収益 (MRR)</CardTitle>
                          <div className="text-2xl font-bold">¥4,820,000</div>
                        </CardHeader>
                        <CardContent>
                          <Badge variant="outline" className="text-emerald-600 border-emerald-500/30 gap-1 text-[11px]">
                            <TrendingUp className="h-3 w-3" /> +12.4% 前月比
                          </Badge>
                        </CardContent>
                      </Card>
                      <Card className="bg-background">
                        <CardHeader className="pb-2">
                          <CardTitle className="text-xs font-medium text-muted-foreground">アクティブ組織</CardTitle>
                          <div className="text-2xl font-bold">1,248</div>
                        </CardHeader>
                        <CardContent>
                          <Badge variant="outline" className="text-emerald-600 border-emerald-500/30 gap-1 text-[11px]">
                            <TrendingUp className="h-3 w-3" /> +8.1% 前月比
                          </Badge>
                        </CardContent>
                      </Card>
                      <Card className="bg-background">
                        <CardHeader className="pb-2">
                          <CardTitle className="text-xs font-medium text-muted-foreground">平均応答速度</CardTitle>
                          <div className="text-2xl font-bold">182 ms</div>
                        </CardHeader>
                        <CardContent>
                          <Badge variant="secondary" className="text-[11px]">
                            安定稼働中 (99.9%)
                          </Badge>
                        </CardContent>
                      </Card>
                    </div>
                  )}
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  実務でそのまま使える売上・アクティブ指標の3連KPIカード
                </CardFooter>
              </Card>
            )}

            {/* Block 2: Search & Filter Bar */}
            {("block filter search bar".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="md:col-span-2 border-primary/30 shadow-xs">
                {renderCardHeader(
                  "Filter & Search Bar Block",
                  "UI Block",
                  "@/components/ui/input, select, button",
                  `import { Input } from "@/components/ui/input"\nimport { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"\nimport { Button } from "@/components/ui/button"`,
                  `<div className="flex flex-wrap items-center gap-3">...</div>`,
                  "テーブルや一覧画面の上部に配置する検索・フィルター操作バー",
                  "block-filter"
                )}
                <CardContent>
                  {codeViewMap["block-filter"] ? (
                    <pre className="text-[11px] font-mono bg-muted p-3 rounded-md overflow-x-auto">
{`<div className="flex flex-col sm:flex-row items-center gap-2 p-3 bg-muted/30 rounded-lg border">
  <Input placeholder="ユーザー名、メールで検索..." className="h-9 w-full sm:w-64" />
  <Select defaultValue="all">
    <SelectTrigger className="h-9 w-full sm:w-40"><SelectValue placeholder="ロール" /></SelectTrigger>
    <SelectContent>
      <SelectItem value="all">すべての権限</SelectItem>
      <SelectItem value="admin">管理者 (Admin)</SelectItem>
    </SelectContent>
  </Select>
  <Button size="sm" className="h-9 gap-1.5 ml-auto"><Plus className="size-4" /> ユーザー追加</Button>
</div>`}
                    </pre>
                  ) : (
                    <div className="flex flex-col sm:flex-row items-center gap-2.5 p-3.5 bg-muted/20 rounded-lg border">
                      <div className="relative w-full sm:w-72">
                        <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                        <Input placeholder="ユーザー名・メールで検索..." className="h-9 pl-8 text-xs bg-background" />
                      </div>
                      <Select defaultValue="all">
                        <SelectTrigger className="h-9 w-full sm:w-36 text-xs bg-background">
                          <SelectValue placeholder="ステータス" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="all">すべて</SelectItem>
                          <SelectItem value="active">Active</SelectItem>
                          <SelectItem value="pending">Pending</SelectItem>
                          <SelectItem value="disabled">Disabled</SelectItem>
                        </SelectContent>
                      </Select>
                      <Button variant="outline" size="sm" className="h-9 text-xs gap-1 w-full sm:w-auto">
                        <Filter className="h-3.5 w-3.5" />
                        詳細絞り込み
                      </Button>
                      <Button size="sm" className="h-9 text-xs gap-1.5 w-full sm:w-auto sm:ml-auto">
                        <Plus className="h-3.5 w-3.5" />
                        新規登録
                      </Button>
                    </div>
                  )}
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  一覧画面のヘッダーに配置する検索窓・セレクト・アクションボタンの統合バー
                </CardFooter>
              </Card>
            )}

            {/* Block 3: Settings Form Block */}
            {("block settings form profile".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Profile Settings Block",
                  "UI Block",
                  "@/components/ui/card, input, switch, avatar",
                  `import { Card, CardHeader, CardContent, CardFooter } from "@/components/ui/card"`,
                  `<Card><CardHeader>...</CardHeader></Card>`,
                  "アバター、名前、通知スイッチを含むプロファイル設定カード",
                  "block-settings"
                )}
                <CardContent>
                  <div className="space-y-4">
                    <div className="flex items-center gap-3">
                      <Avatar size="lg">
                        <AvatarFallback className="bg-primary text-primary-foreground font-semibold">TK</AvatarFallback>
                      </Avatar>
                      <div className="space-y-1">
                        <Button size="xs" variant="outline">画像をアップロード</Button>
                        <p className="text-[11px] text-muted-foreground">JPG, PNG (最大 2MB)</p>
                      </div>
                    </div>
                    <div className="space-y-2">
                      <Label htmlFor="block-name" className="text-xs">表示名</Label>
                      <Input id="block-name" defaultValue="Taiki Noda" className="h-8 text-xs" />
                    </div>
                    <div className="flex items-center justify-between rounded-md border p-2.5">
                      <div className="space-y-0.5">
                        <Label htmlFor="block-notify" className="text-xs font-medium">メール通知</Label>
                        <p className="text-[10px] text-muted-foreground">重要イベントをメールで受け取る</p>
                      </div>
                      <Switch id="block-notify" defaultChecked />
                    </div>
                    <Button size="sm" className="w-full text-xs" onClick={() => toast.success("プロフィール設定を保存しました")}>
                      変更を保存
                    </Button>
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  実戦的なユーザー情報編集フォーム
                </CardFooter>
              </Card>
            )}

            {/* Block 4: Empty State */}
            {("block empty state".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Empty State Block",
                  "UI Block",
                  "@/components/ui/card, button",
                  `import { Card, CardContent } from "@/components/ui/card"`,
                  `<div className="text-center py-8">...</div>`,
                  "データ未登録時や検索結果ゼロ件時に表示する統一EmptyState画面",
                  "block-empty"
                )}
                <CardContent>
                  <div className="flex flex-col items-center justify-center text-center py-6 px-4 border border-dashed rounded-lg bg-muted/10">
                    <div className="flex size-10 items-center justify-center rounded-full bg-muted mb-3 text-muted-foreground">
                      <Inbox className="size-5" />
                    </div>
                    <h4 className="text-sm font-semibold">プロジェクトが見つかりません</h4>
                    <p className="text-xs text-muted-foreground mt-1 max-w-xs">
                      まだリポジトリやスキャフォールドが作成されていません。新しいプロジェクトを開始しましょう。
                    </p>
                    <Button size="sm" className="mt-4 gap-1.5 text-xs" onClick={() => toast.info("新規作成ダイアログを開きます")}>
                      <Plus className="size-3.5" />
                      新規プロジェクト作成
                    </Button>
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  AIが独自に崩れやすい「データ未登録画面」の標準化パターン
                </CardFooter>
              </Card>
            )}
          </>
        )}

        {/* ========================================================= */}
        {/* 1. BUTTONS & BADGES */}
        {/* ========================================================= */}
        {(selectedCategory === "all" || selectedCategory === "buttons") && (
          <>
            {/* Button */}
            {("button".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Button",
                  "Action",
                  "@/components/ui/button.tsx",
                  `import { Button } from "@/components/ui/button"`,
                  `<Button variant="default" size="sm">Click Me</Button>`,
                  "各種ボタントリガーやフォーム送信アクション",
                  "btn-comp"
                )}
                <CardContent className="space-y-4">
                  {codeViewMap["btn-comp"] ? (
                    <pre className="text-[11px] font-mono bg-muted p-3 rounded-md overflow-x-auto">
{`<div className="flex flex-wrap gap-2">
  <Button size="sm">Default</Button>
  <Button size="sm" variant="secondary">Secondary</Button>
  <Button size="sm" variant="outline">Outline</Button>
  <Button size="sm" variant="destructive">Destructive</Button>
  <Button size="sm" variant="ghost">Ghost</Button>
</div>`}
                    </pre>
                  ) : (
                    <>
                      <div className="flex flex-wrap items-center gap-2">
                        <Button size="sm">Default</Button>
                        <Button size="sm" variant="secondary">Secondary</Button>
                        <Button size="sm" variant="outline">Outline</Button>
                        <Button size="sm" variant="ghost">Ghost</Button>
                        <Button size="sm" variant="destructive">Destructive</Button>
                      </div>
                      <div className="flex flex-wrap items-center gap-2 pt-2 border-t border-dashed">
                        <Button size="sm" className="gap-1.5">
                          <Sparkles className="h-3.5 w-3.5" />
                          With Icon
                        </Button>
                        <Button size="sm" variant="outline" disabled>Disabled</Button>
                        <Button size="sm" variant="outline" className="h-8 w-8 p-0">
                          <Settings className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    </>
                  )}
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  バリアント (default, secondary, outline, ghost, destructive) とサイズ展開
                </CardFooter>
              </Card>
            )}

            {/* Badge */}
            {("badge".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Badge",
                  "Status",
                  "@/components/ui/badge.tsx",
                  `import { Badge } from "@/components/ui/badge"`,
                  `<Badge variant="outline">Live Status</Badge>`,
                  "ステータス表示やタグ、バージョンラベル",
                  "badge-comp"
                )}
                <CardContent className="space-y-4">
                  {codeViewMap["badge-comp"] ? (
                    <pre className="text-[11px] font-mono bg-muted p-3 rounded-md overflow-x-auto">
{`<Badge>Default</Badge>
<Badge variant="secondary">Secondary</Badge>
<Badge variant="outline">Outline</Badge>
<Badge variant="destructive">Destructive</Badge>`}
                    </pre>
                  ) : (
                    <>
                      <div className="flex flex-wrap items-center gap-2">
                        <Badge>Default</Badge>
                        <Badge variant="secondary">Secondary</Badge>
                        <Badge variant="outline">Outline</Badge>
                        <Badge variant="destructive">Destructive</Badge>
                      </div>
                      <div className="flex flex-wrap items-center gap-2 pt-2 border-t border-dashed">
                        <Badge variant="outline" className="gap-1 text-xs">
                          <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse" />
                          Live Status
                        </Badge>
                        <Badge variant="secondary" className="text-xs">v2.4.0</Badge>
                      </div>
                    </>
                  )}
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  ステータス表示やタグ付けに適した軽量バッジ
                </CardFooter>
              </Card>
            )}
          </>
        )}

        {/* ========================================================= */}
        {/* 2. FORMS & INPUTS (新コンポーネント含む) */}
        {/* ========================================================= */}
        {(selectedCategory === "all" || selectedCategory === "forms") && (
          <>
            {/* Input & Label */}
            {("input label".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Input & Label",
                  "Form",
                  "@/components/ui/input.tsx, label.tsx",
                  `import { Input } from "@/components/ui/input"\nimport { Label } from "@/components/ui/label"`,
                  `<div className="space-y-1.5">\n  <Label htmlFor="email">Email</Label>\n  <Input id="email" type="email" />\n</div>`,
                  "テキスト入力フィールドとアクセシブルラベル",
                  "input-comp"
                )}
                <CardContent className="space-y-3">
                  <div className="space-y-1.5">
                    <Label htmlFor="cat-email">Email Address</Label>
                    <Input id="cat-email" type="email" placeholder="alex@example.com" />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="cat-disabled" className="text-muted-foreground">Disabled Field</Label>
                    <Input id="cat-disabled" disabled placeholder="Disabled input" />
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  アクセシビリティ対応の標準テキストインプットとラベル
                </CardFooter>
              </Card>
            )}

            {/* Textarea */}
            {("textarea".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Textarea",
                  "Form",
                  "@/components/ui/textarea.tsx",
                  `import { Textarea } from "@/components/ui/textarea"`,
                  `<Textarea placeholder="Type something..." rows={3} />`,
                  "複数行のテキスト入力や長文コメント欄",
                  "textarea-comp"
                )}
                <CardContent className="space-y-2">
                  <Label htmlFor="cat-textarea">Project Description</Label>
                  <Textarea
                    id="cat-textarea"
                    placeholder="Describe your architecture, data flow, and requirements..."
                    rows={3}
                  />
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  複数行テキスト入力（自動伸縮・高さ調整対応）
                </CardFooter>
              </Card>
            )}

            {/* Select */}
            {("select".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Select",
                  "Form",
                  "@/components/ui/select.tsx",
                  `import {\n  Select,\n  SelectContent,\n  SelectGroup,\n  SelectItem,\n  SelectLabel,\n  SelectTrigger,\n  SelectValue,\n} from "@/components/ui/select"`,
                  `<Select defaultValue="nextjs">\n  <SelectTrigger><SelectValue /></SelectTrigger>\n  <SelectContent><SelectItem value="nextjs">Next.js</SelectItem></SelectContent>\n</Select>`,
                  "ドロップダウンリストからの単一選択メニュー",
                  "select-comp"
                )}
                <CardContent className="space-y-2">
                  <Label>Frontend Framework</Label>
                  <Select defaultValue="nextjs">
                    <SelectTrigger className="w-full">
                      <SelectValue placeholder="フレームワークを選択" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectLabel>React Ecosystem</SelectLabel>
                        <SelectItem value="nextjs">Next.js 16 (App Router)</SelectItem>
                        <SelectItem value="remix">Remix / React Router</SelectItem>
                        <SelectItem value="astro">Astro</SelectItem>
                        <SelectItem value="vite">Vite + React SPA</SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  キーボードナビゲーション完全対応のドロップダウンセレクタ
                </CardFooter>
              </Card>
            )}

            {/* Slider (NEW!) */}
            {("slider range".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Slider (New)",
                  "Form",
                  "@/components/ui/slider.tsx",
                  `import { Slider } from "@/components/ui/slider"`,
                  `<Slider defaultValue={[50]} max={100} step={1} />`,
                  "音量・予算・数値範囲の直感的なスライダー入力",
                  "slider-comp"
                )}
                <CardContent className="space-y-4">
                  <div className="space-y-2">
                    <div className="flex justify-between text-xs">
                      <Label>Token Limit Budget</Label>
                      <span className="font-mono text-muted-foreground">{sliderVal[0]}%</span>
                    </div>
                    <Slider
                      value={sliderVal}
                      onValueChange={(val) => setSliderVal(Array.isArray(val) ? val : [val])}
                      max={100}
                      step={1}
                    />
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  ドラッグおよびキーボード操作に対応した数値スライダー
                </CardFooter>
              </Card>
            )}

            {/* Radio Group (NEW!) */}
            {("radio group".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Radio Group (New)",
                  "Form",
                  "@/components/ui/radio-group.tsx",
                  `import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"`,
                  `<RadioGroup defaultValue="standard">\n  <div className="flex items-center gap-2">\n    <RadioGroupItem value="standard" id="r1" />\n    <Label htmlFor="r1">Standard</Label>\n  </div>\n</RadioGroup>`,
                  "複数の選択肢から1つを選択するラジオボタン群",
                  "radio-comp"
                )}
                <CardContent>
                  <RadioGroup defaultValue="token-economy">
                    <div className="flex items-center space-x-2">
                      <RadioGroupItem value="token-economy" id="r-opt1" />
                      <Label htmlFor="r-opt1" className="text-xs">トークン節約モード (Scaffold優先)</Label>
                    </div>
                    <div className="flex items-center space-x-2">
                      <RadioGroupItem value="full-custom" id="r-opt2" />
                      <Label htmlFor="r-opt2" className="text-xs">フルカスタムモード (独自CSS許可)</Label>
                    </div>
                  </RadioGroup>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  グループ化されたセマンティックなラジオボタン
                </CardFooter>
              </Card>
            )}

            {/* Switch & Checkbox */}
            {("switch checkbox".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Switch & Checkbox",
                  "Form",
                  "@/components/ui/switch.tsx, checkbox.tsx",
                  `import { Switch } from "@/components/ui/switch"\nimport { Checkbox } from "@/components/ui/checkbox"`,
                  `<Switch defaultChecked />\n<Checkbox id="terms" />`,
                  "トグルスイッチと利用規約などの同意チェックボックス",
                  "switch-comp"
                )}
                <CardContent className="space-y-4">
                  <div className="flex items-center justify-between rounded-lg border p-2.5">
                    <div className="space-y-0.5">
                      <Label htmlFor="cat-switch" className="text-xs font-medium">Auto Token Optimization</Label>
                      <p className="text-[10px] text-muted-foreground">自動でUI定義トークンを削減します</p>
                    </div>
                    <Switch id="cat-switch" defaultChecked />
                  </div>
                  <div className="flex items-center space-x-2 pt-1">
                    <Checkbox id="cat-terms" defaultChecked />
                    <Label htmlFor="cat-terms" className="text-xs font-normal">
                      利用規約およびプライバシーポリシーに同意する
                    </Label>
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  直感的なON/OFFトグルと複数選択・同意用チェックボックス
                </CardFooter>
              </Card>
            )}
          </>
        )}

        {/* ========================================================= */}
        {/* 3. OVERLAYS & MENUS */}
        {/* ========================================================= */}
        {(selectedCategory === "all" || selectedCategory === "overlays") && (
          <>
            {/* Dialog */}
            {("dialog modal".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Dialog (Modal)",
                  "Overlay",
                  "@/components/ui/dialog.tsx",
                  `import {\n  Dialog,\n  DialogContent,\n  DialogDescription,\n  DialogFooter,\n  DialogHeader,\n  DialogTitle,\n  DialogTrigger,\n} from "@/components/ui/dialog"`,
                  `<Dialog><DialogTrigger>Open</DialogTrigger><DialogContent>...</DialogContent></Dialog>`,
                  "確認モーダルやポップアップ編集ウィンドウ",
                  "dialog-comp"
                )}
                <CardContent className="flex flex-col items-start gap-2">
                  <p className="text-xs text-muted-foreground">
                    ユーザーの確認や重要な操作を促すモーダルウィンドウ
                  </p>
                  <Dialog>
                    <DialogTrigger className={cn(buttonVariants({ variant: "outline", size: "sm" }), "cursor-pointer")}>
                      Open Dialog Demo
                    </DialogTrigger>
                    <DialogContent>
                      <DialogHeader>
                        <DialogTitle>Deploy to Production?</DialogTitle>
                        <DialogDescription>
                          この操作を実行すると、変更が即座にライブ環境へ反映されます。続行してもよろしいですか？
                        </DialogDescription>
                      </DialogHeader>
                      <DialogFooter className="gap-2">
                        <Button variant="outline" size="sm" onClick={() => toast.info("デプロイを中止しました")}>キャンセル</Button>
                        <Button size="sm" onClick={() => toast.success("デプロイを開始しました！")}>デプロイ実行</Button>
                      </DialogFooter>
                    </DialogContent>
                  </Dialog>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  フォーカストラップとEscキー対応のフルアクセシブルダイアログ
                </CardFooter>
              </Card>
            )}

            {/* Sheet */}
            {("sheet drawer".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Sheet (Drawer)",
                  "Overlay",
                  "@/components/ui/sheet.tsx",
                  `import {\n  Sheet,\n  SheetContent,\n  SheetDescription,\n  SheetHeader,\n  SheetTitle,\n  SheetTrigger,\n} from "@/components/ui/sheet"`,
                  `<Sheet><SheetTrigger>Open Drawer</SheetTrigger><SheetContent side="right">...</SheetContent></Sheet>`,
                  "画面端からスライドするサイドドロワーや詳細設定パネル",
                  "sheet-comp"
                )}
                <CardContent className="flex flex-col items-start gap-2">
                  <p className="text-xs text-muted-foreground">
                    画面端からスライドインするサイドパネル（モバイルメニューや詳細設定に最適）
                  </p>
                  <Sheet>
                    <SheetTrigger className={cn(buttonVariants({ variant: "outline", size: "sm" }), "cursor-pointer gap-1.5")}>
                      <PanelLeft className="h-4 w-4" />
                      Open Side Sheet
                    </SheetTrigger>
                    <SheetContent side="right">
                      <SheetHeader>
                        <SheetTitle>Project Settings</SheetTitle>
                        <SheetDescription>
                          サイドドロワー内でプロジェクトの詳細設定やメタデータを編集できます。
                        </SheetDescription>
                      </SheetHeader>
                      <div className="space-y-4 py-4">
                        <div className="space-y-1.5">
                          <Label className="text-xs">App Name</Label>
                          <Input defaultValue="NextScaffold Demo" className="h-8 text-xs" />
                        </div>
                        <div className="space-y-1.5">
                          <Label className="text-xs">API Environment</Label>
                          <Input defaultValue="https://api.production.example" className="h-8 text-xs" />
                        </div>
                      </div>
                      <SheetFooter>
                        <SheetClose className={cn(buttonVariants({ size: "sm" }))} onClick={() => toast.success("Settings saved")}>
                          Save Changes
                        </SheetClose>
                      </SheetFooter>
                    </SheetContent>
                  </Sheet>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  上下左右（top/bottom/left/right）からスライドイン可能なドロワー
                </CardFooter>
              </Card>
            )}

            {/* Dropdown Menu */}
            {("dropdown menu".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Dropdown Menu",
                  "Overlay",
                  "@/components/ui/dropdown-menu.tsx",
                  `import {\n  DropdownMenu,\n  DropdownMenuContent,\n  DropdownMenuItem,\n  DropdownMenuTrigger,\n} from "@/components/ui/dropdown-menu"`,
                  `<DropdownMenu><DropdownMenuTrigger>Menu</DropdownMenuTrigger><DropdownMenuContent>...</DropdownMenuContent></DropdownMenu>`,
                  "アクションメニューやユーザー設定ドロップダウン",
                  "dropdown-comp"
                )}
                <CardContent className="flex flex-col items-start gap-2">
                  <p className="text-xs text-muted-foreground">
                    ユーザーアクションやコンテキストメニュー用の展開リスト
                  </p>
                  <DropdownMenu>
                    <DropdownMenuTrigger className={cn(buttonVariants({ variant: "outline", size: "sm" }), "cursor-pointer gap-1.5")}>
                      <User className="h-4 w-4" />
                      Account Actions
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start" className="w-56">
                      <DropdownMenuLabel>My Account</DropdownMenuLabel>
                      <DropdownMenuSeparator />
                      <DropdownMenuGroup>
                        <DropdownMenuItem onClick={() => toast.info("Profile selected")}>
                          <User className="mr-2 h-4 w-4" />
                          <span>Profile</span>
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => toast.info("Settings selected")}>
                          <Settings className="mr-2 h-4 w-4" />
                          <span>Settings</span>
                        </DropdownMenuItem>
                      </DropdownMenuGroup>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem
                        variant="destructive"
                        onClick={() => toast.error("Logged out")}
                      >
                        <LogOut className="mr-2 h-4 w-4" />
                        <span>Log out</span>
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  ラベル、セパレータ、Destructiveアクション対応メニュー
                </CardFooter>
              </Card>
            )}

            {/* Popover */}
            {("popover".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Popover",
                  "Overlay",
                  "@/components/ui/popover.tsx",
                  `import {\n  Popover,\n  PopoverContent,\n  PopoverTrigger,\n} from "@/components/ui/popover"`,
                  `<Popover><PopoverTrigger>Open</PopoverTrigger><PopoverContent>...</PopoverContent></Popover>`,
                  "要素クリック時に開くリッチなフロートパネル",
                  "popover-comp"
                )}
                <CardContent className="flex flex-col items-start gap-2">
                  <p className="text-xs text-muted-foreground">
                    クリックした要素の近くに浮かび上がるリッチなコンテンツパネル
                  </p>
                  <Popover>
                    <PopoverTrigger className={cn(buttonVariants({ variant: "outline", size: "sm" }), "cursor-pointer gap-1.5")}>
                      <Sliders className="h-4 w-4" />
                      Adjust Canvas Size
                    </PopoverTrigger>
                    <PopoverContent className="w-80">
                      <div className="grid gap-4">
                        <div className="space-y-1">
                          <h4 className="font-medium text-sm leading-none">Canvas Dimensions</h4>
                          <p className="text-xs text-muted-foreground">作業スペースの解像度を設定します。</p>
                        </div>
                        <div className="grid gap-2">
                          <div className="grid grid-cols-3 items-center gap-4">
                            <Label htmlFor="p-w" className="text-xs">Width</Label>
                            <Input id="p-w" defaultValue="100%" className="col-span-2 h-8 text-xs" />
                          </div>
                          <div className="grid grid-cols-3 items-center gap-4">
                            <Label htmlFor="p-h" className="text-xs">Height</Label>
                            <Input id="p-h" defaultValue="auto" className="col-span-2 h-8 text-xs" />
                          </div>
                        </div>
                      </div>
                    </PopoverContent>
                  </Popover>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  入力フォームや詳細設定を埋め込めるリッチポップアップ
                </CardFooter>
              </Card>
            )}

            {/* Tooltip */}
            {("tooltip".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Tooltip",
                  "Overlay",
                  "@/components/ui/tooltip.tsx",
                  `import {\n  Tooltip,\n  TooltipContent,\n  TooltipTrigger,\n} from "@/components/ui/tooltip"`,
                  `<Tooltip><TooltipTrigger>Hover</TooltipTrigger><TooltipContent>Text</TooltipContent></Tooltip>`,
                  "アイコンやボタンへのホバー時ヒント表示",
                  "tooltip-comp"
                )}
                <CardContent className="flex flex-col items-start gap-3">
                  <p className="text-xs text-muted-foreground">
                    ボタンやアイコンにカーソルを合わせたときに表示されるヒント
                  </p>
                  <div className="flex items-center gap-3">
                    <Tooltip>
                      <TooltipTrigger className={cn(buttonVariants({ variant: "outline", size: "sm" }))}>
                        Hover for Help
                      </TooltipTrigger>
                      <TooltipContent>
                        <p>トークン削減用の事前定義スタイルが適用されます</p>
                      </TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger className={cn(buttonVariants({ variant: "ghost", size: "icon" }), "h-8 w-8")}>
                        <Info className="h-4 w-4 text-muted-foreground" />
                      </TooltipTrigger>
                      <TooltipContent side="right">
                        <p>クイックインフォメーション</p>
                      </TooltipContent>
                    </Tooltip>
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  矢印ポインター付きの滑らかなアニメーションツールチップ
                </CardFooter>
              </Card>
            )}
          </>
        )}

        {/* ========================================================= */}
        {/* 4. DATA DISPLAY (NEW: Alert, Progress, Pagination) */}
        {/* ========================================================= */}
        {(selectedCategory === "all" || selectedCategory === "data-display") && (
          <>
            {/* Alert (NEW!) */}
            {("alert banner warning notice".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between md:col-span-2">
                {renderCardHeader(
                  "Alert (New)",
                  "Data Display",
                  "@/components/ui/alert.tsx",
                  `import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert"`,
                  `<Alert variant="info">\n  <AlertTitle>Notice</AlertTitle>\n  <AlertDescription>System update scheduled</AlertDescription>\n</Alert>`,
                  "システム警告、情報バナー、エラー通知ボックス",
                  "alert-comp"
                )}
                <CardContent className="grid gap-3 sm:grid-cols-2">
                  <Alert variant="info">
                    <Info className="size-4" />
                    <AlertTitle>新機能が利用可能</AlertTitle>
                    <AlertDescription>
                      Next.js 16 App RouterとTailwind v4の最適化が完了しました。
                    </AlertDescription>
                  </Alert>
                  <Alert variant="destructive">
                    <AlertCircle className="size-4" />
                    <AlertTitle>トークン枯渇の警告</AlertTitle>
                    <AlertDescription>
                      生のCSSをAIに出力させるとトークンを大量消費します。既存パーツを使ってください。
                    </AlertDescription>
                  </Alert>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  default, info, destructive, success バリアントに対応したインラインアラート
                </CardFooter>
              </Card>
            )}

            {/* Progress (NEW!) */}
            {("progress bar loading".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Progress (New)",
                  "Data Display",
                  "@/components/ui/progress.tsx",
                  `import { Progress } from "@/components/ui/progress"`,
                  `<Progress value={68} max={100} />`,
                  "タスクの進行度やストレージ使用率プログレスバー",
                  "progress-comp"
                )}
                <CardContent className="space-y-4">
                  <div className="space-y-2">
                    <div className="flex justify-between text-xs">
                      <span>アップロード進捗</span>
                      <span className="font-mono text-muted-foreground">{progressVal}%</span>
                    </div>
                    <Progress value={progressVal} />
                  </div>
                  <div className="flex gap-2">
                    <Button size="xs" variant="outline" onClick={() => setProgressVal(Math.max(0, progressVal - 15))}>-15%</Button>
                    <Button size="xs" variant="outline" onClick={() => setProgressVal(Math.min(100, progressVal + 15))}>+15%</Button>
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  スムーズなトランジションアニメーション付きプログレスバー
                </CardFooter>
              </Card>
            )}

            {/* Pagination (NEW!) */}
            {("pagination page next prev".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Pagination (New)",
                  "Navigation",
                  "@/components/ui/pagination.tsx",
                  `import {\n  Pagination,\n  PaginationContent,\n  PaginationItem,\n  PaginationLink,\n  PaginationNext,\n  PaginationPrevious,\n} from "@/components/ui/pagination"`,
                  `<Pagination><PaginationContent>...</PaginationContent></Pagination>`,
                  "テーブルや一覧表示のページネーションコントロール",
                  "pagination-comp"
                )}
                <CardContent>
                  <Pagination>
                    <PaginationContent>
                      <PaginationItem>
                        <PaginationPrevious href="#components" />
                      </PaginationItem>
                      <PaginationItem>
                        <PaginationLink href="#components" isActive>1</PaginationLink>
                      </PaginationItem>
                      <PaginationItem>
                        <PaginationLink href="#components">2</PaginationLink>
                      </PaginationItem>
                      <PaginationItem>
                        <PaginationEllipsis />
                      </PaginationItem>
                      <PaginationItem>
                        <PaginationNext href="#components" />
                      </PaginationItem>
                    </PaginationContent>
                  </Pagination>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  アクティブ状態・省略記号対応のアクセシブルページネーション
                </CardFooter>
              </Card>
            )}

            {/* Avatar */}
            {("avatar user".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Avatar",
                  "Data Display",
                  "@/components/ui/avatar.tsx",
                  `import {\n  Avatar,\n  AvatarFallback,\n  AvatarImage,\n  AvatarBadge,\n  AvatarGroup,\n} from "@/components/ui/avatar"`,
                  `<Avatar><AvatarFallback>JD</AvatarFallback></Avatar>`,
                  "ユーザーのプロフィールアイコンやメンバーグループ一覧",
                  "avatar-comp"
                )}
                <CardContent className="space-y-4">
                  <div className="flex items-center gap-4">
                    <Avatar size="lg">
                      <AvatarImage src="https://images.unsplash.com/photo-1534528741775-53994a69daeb?w=100&auto=format&fit=crop&q=80" />
                      <AvatarFallback>TK</AvatarFallback>
                      <AvatarBadge className="bg-emerald-500" />
                    </Avatar>
                    <Avatar size="default">
                      <AvatarFallback>AI</AvatarFallback>
                    </Avatar>
                    <Avatar size="sm">
                      <AvatarFallback>NS</AvatarFallback>
                    </Avatar>
                  </div>
                  <div className="pt-2 border-t border-dashed">
                    <p className="text-xs text-muted-foreground mb-2">Avatar Group（複数メンバー表示）:</p>
                    <AvatarGroup>
                      <Avatar><AvatarFallback className="bg-primary/10 text-primary">JD</AvatarFallback></Avatar>
                      <Avatar><AvatarFallback className="bg-primary/20 text-primary">AK</AvatarFallback></Avatar>
                      <Avatar><AvatarFallback className="bg-primary/30 text-primary">ST</AvatarFallback></Avatar>
                      <AvatarGroupCount>+5</AvatarGroupCount>
                    </AvatarGroup>
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  画像・フォールバック頭文字・オンラインバッジ・グループ表記対応
                </CardFooter>
              </Card>
            )}

            {/* Breadcrumb */}
            {("breadcrumb navigation".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Breadcrumb",
                  "Navigation",
                  "@/components/ui/breadcrumb.tsx",
                  `import {\n  Breadcrumb,\n  BreadcrumbItem,\n  BreadcrumbLink,\n  BreadcrumbList,\n  BreadcrumbPage,\n  BreadcrumbSeparator,\n} from "@/components/ui/breadcrumb"`,
                  `<Breadcrumb><BreadcrumbList>...</BreadcrumbList></Breadcrumb>`,
                  "階層的なパンくずナビゲーションバー",
                  "bc-comp"
                )}
                <CardContent className="space-y-4">
                  <Breadcrumb>
                    <BreadcrumbList>
                      <BreadcrumbItem><BreadcrumbLink href="#">Home</BreadcrumbLink></BreadcrumbItem>
                      <BreadcrumbSeparator />
                      <BreadcrumbItem><BreadcrumbLink href="#components">Scaffold</BreadcrumbLink></BreadcrumbItem>
                      <BreadcrumbSeparator />
                      <BreadcrumbItem><BreadcrumbPage>Components</BreadcrumbPage></BreadcrumbItem>
                    </BreadcrumbList>
                  </Breadcrumb>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  階層構造を直感的に示すセマンティックなパンくずナビゲーション
                </CardFooter>
              </Card>
            )}

            {/* Table */}
            {("table grid data".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between md:col-span-2">
                {renderCardHeader(
                  "Table",
                  "Data Display",
                  "@/components/ui/table.tsx",
                  `import {\n  Table,\n  TableBody,\n  TableCell,\n  TableHead,\n  TableHeader,\n  TableRow,\n} from "@/components/ui/table"`,
                  `<Table><TableHeader>...</TableHeader><TableBody>...</TableBody></Table>`,
                  "トランザクションやユーザー一覧などのデータテーブル",
                  "table-comp"
                )}
                <CardContent>
                  <div className="rounded-md border overflow-hidden">
                    <Table>
                      <TableHeader>
                        <TableRow>
                          <TableHead className="w-[180px]">Component Name</TableHead>
                          <TableHead>Category</TableHead>
                          <TableHead>Base Library</TableHead>
                          <TableHead className="text-right">Status</TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        <TableRow>
                          <TableCell className="font-medium text-xs">Button</TableCell>
                          <TableCell className="text-xs">Form / Action</TableCell>
                          <TableCell className="text-xs text-muted-foreground">cva + tailwind</TableCell>
                          <TableCell className="text-right"><Badge variant="outline" className="text-[10px]">Ready</Badge></TableCell>
                        </TableRow>
                        <TableRow>
                          <TableCell className="font-medium text-xs">Dialog</TableCell>
                          <TableCell className="text-xs">Overlay</TableCell>
                          <TableCell className="text-xs text-muted-foreground">@base-ui/react</TableCell>
                          <TableCell className="text-right"><Badge variant="outline" className="text-[10px]">Ready</Badge></TableCell>
                        </TableRow>
                        <TableRow>
                          <TableCell className="font-medium text-xs">Alert (New)</TableCell>
                          <TableCell className="text-xs">Data Display</TableCell>
                          <TableCell className="text-xs text-muted-foreground">cva + tailwind</TableCell>
                          <TableCell className="text-right"><Badge variant="secondary" className="text-[10px]">Added</Badge></TableCell>
                        </TableRow>
                      </TableBody>
                    </Table>
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  ヘッダー固定・スクロール対応のセマンティックデータテーブル
                </CardFooter>
              </Card>
            )}

            {/* Accordion */}
            {("accordion faq collapse".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Accordion",
                  "Data Display",
                  "@/components/ui/accordion.tsx",
                  `import {\n  Accordion,\n  AccordionContent,\n  AccordionItem,\n  AccordionTrigger,\n} from "@/components/ui/accordion"`,
                  `<Accordion><AccordionItem value="1">...</AccordionItem></Accordion>`,
                  "FAQやヘルプセクションの折りたたみアコーディオン",
                  "acc-comp"
                )}
                <CardContent>
                  <Accordion className="w-full">
                    <AccordionItem value="acc-1">
                      <AccordionTrigger className="text-sm">なぜコンポーネントカタログが必要？</AccordionTrigger>
                      <AccordionContent className="text-xs text-muted-foreground">
                        同梱されているUIパーツを一覧化し、AIや人間が迷わず再利用することでトークン消費を抑え、デザインの破綻を防ぎます。
                      </AccordionContent>
                    </AccordionItem>
                    <AccordionItem value="acc-2">
                      <AccordionTrigger className="text-sm">Tailwind v4に対応している？</AccordionTrigger>
                      <AccordionContent className="text-xs text-muted-foreground">
                        はい。最新の Tailwind CSS v4 と @base-ui/react をベースに最適化されています。
                      </AccordionContent>
                    </AccordionItem>
                  </Accordion>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  スムーズな開閉アニメーションを持つ折りたたみリスト
                </CardFooter>
              </Card>
            )}

            {/* Skeleton */}
            {("skeleton loading".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Skeleton",
                  "Loading",
                  "@/components/ui/skeleton.tsx",
                  `import { Skeleton } from "@/components/ui/skeleton"`,
                  `<Skeleton className="h-4 w-full" />`,
                  "APIデータ取得中のローディングプレースホルダー骨格",
                  "skel-comp"
                )}
                <CardContent className="space-y-3">
                  <div className="flex items-center space-x-4">
                    <Skeleton className="h-10 w-10 rounded-full" />
                    <div className="space-y-2 flex-1">
                      <Skeleton className="h-4 w-3/4" />
                      <Skeleton className="h-3 w-1/2" />
                    </div>
                  </div>
                  <Skeleton className="h-10 w-full rounded-md" />
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  データ読み込み中のプレースホルダーアニメーション
                </CardFooter>
              </Card>
            )}

            {/* Separator */}
            {("separator divider".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Separator",
                  "Structure",
                  "@/components/ui/separator.tsx",
                  `import { Separator } from "@/components/ui/separator"`,
                  `<Separator />\n<Separator orientation="vertical" />`,
                  "コンテンツやヘッダーの水平・垂直区切り線",
                  "sep-comp"
                )}
                <CardContent className="space-y-4">
                  <div className="space-y-1">
                    <p className="text-xs font-medium">Horizontal Separator</p>
                    <Separator className="my-2" />
                    <p className="text-xs text-muted-foreground">セクションの分割</p>
                  </div>
                  <div className="flex h-5 items-center space-x-4 text-xs">
                    <span>Overview</span>
                    <Separator orientation="vertical" />
                    <span>Analytics</span>
                    <Separator orientation="vertical" />
                    <span>Settings</span>
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  水平（horizontal）・垂直（vertical）方向の区切り線
                </CardFooter>
              </Card>
            )}
          </>
        )}

        {/* ========================================================= */}
        {/* 5. FEEDBACK & TABS */}
        {/* ========================================================= */}
        {(selectedCategory === "all" || selectedCategory === "feedback") && (
          <>
            {/* Sonner (Toast) */}
            {("sonner toast notification".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Sonner (Toast)",
                  "Notification",
                  "@/components/ui/sonner.tsx (toast from 'sonner')",
                  `import { toast } from "sonner"`,
                  `toast.success("Saved successfully!")`,
                  "操作成功・エラー・注意喚起のフローティングトースト通知",
                  "toast-comp"
                )}
                <CardContent className="space-y-3">
                  <p className="text-xs text-muted-foreground">スタック表示対応の洗練された軽量トースト通知</p>
                  <div className="flex flex-wrap gap-2">
                    <Button size="sm" variant="outline" onClick={() => toast.success("成功トーストが表示されました！")}>Success</Button>
                    <Button size="sm" variant="outline" onClick={() => toast.error("エラーが発生しました")}>Error</Button>
                    <Button size="sm" variant="outline" onClick={() => toast.info("システム更新が予定されています")}>Info</Button>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() =>
                        toast("イベントが作成されました", {
                          action: { label: "取り消し", onClick: () => toast.info("操作を取り消しました") },
                        })
                      }
                    >
                      With Action
                    </Button>
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  画面右上に滑らかにスライドインするトースト
                </CardFooter>
              </Card>
            )}

            {/* Tabs */}
            {("tabs".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Tabs",
                  "Navigation",
                  "@/components/ui/tabs.tsx",
                  `import {\n  Tabs,\n  TabsContent,\n  TabsList,\n  TabsTrigger,\n} from "@/components/ui/tabs"`,
                  `<Tabs defaultValue="account"><TabsList>...</TabsList><TabsContent>...</TabsContent></Tabs>`,
                  "画面内の複数ビューを切り替えるセグメントタブ",
                  "tabs-comp"
                )}
                <CardContent>
                  <Tabs defaultValue="account" className="w-full">
                    <TabsList className="grid w-full grid-cols-2">
                      <TabsTrigger value="account">Account</TabsTrigger>
                      <TabsTrigger value="password">Password</TabsTrigger>
                    </TabsList>
                    <TabsContent value="account" className="pt-2 text-xs text-muted-foreground">
                      アカウント基本情報やプロファイル設定をここに配置します。
                    </TabsContent>
                    <TabsContent value="password" className="pt-2 text-xs text-muted-foreground">
                      パスワード変更や2要素認証の設定画面です。
                    </TabsContent>
                  </Tabs>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  コンテンツを切り替えるセグメントタブUI
                </CardFooter>
              </Card>
            )}
          </>
        )}

        {/* ========================================================= */}
        {/* 6. LAYOUTS & THEME */}
        {/* ========================================================= */}
        {(selectedCategory === "all" || selectedCategory === "layouts") && (
          <>
            {/* Theme Toggle & Provider */}
            {("theme toggle provider darkmode".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "ThemeToggle & Provider",
                  "Theme",
                  "@/components/theme-toggle.tsx, theme-provider.tsx",
                  `import { ThemeToggle } from "@/components/theme-toggle"`,
                  `<ThemeToggle />`,
                  "ダークモード・ライトモード切り替えボタン",
                  "theme-comp"
                )}
                <CardContent className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <p className="text-xs font-medium">Dark / Light Mode</p>
                      <p className="text-[11px] text-muted-foreground">クリックしてダーク/ライトをトグル</p>
                    </div>
                    <ThemeToggle />
                  </div>
                  <div className="flex items-center justify-between pt-2 border-t border-dashed">
                    <div className="space-y-0.5">
                      <p className="text-xs font-medium">Color Preset (ライブ切替)</p>
                      <p className="text-[11px] text-muted-foreground">Blue, Emerald, Violet, Rose 等</p>
                    </div>
                    <ColorThemeToggle />
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  システム連動のダークモードと、CSS変数による動的カラープリセット
                </CardFooter>
              </Card>
            )}

            {/* Layouts Overview */}
            {("layout dashboard auth marketing".includes(searchQuery.toLowerCase()) || !searchQuery) && (
              <Card className="flex flex-col justify-between">
                {renderCardHeader(
                  "Built-in Layouts",
                  "Template",
                  "@/components/layouts/*",
                  `import { DashboardLayout } from "@/components/layouts/dashboard-layout"`,
                  `<DashboardLayout title="Overview">{children}</DashboardLayout>`,
                  "ダッシュボード、認証、マーケティングの共通レイアウト骨格",
                  "layout-comp"
                )}
                <CardContent className="space-y-2">
                  <p className="text-xs text-muted-foreground">
                    ゼロから外枠レイアウトを作らせず、事前配備された型を適用することでトークンを節約します。
                  </p>
                  <div className="grid grid-cols-2 gap-2 pt-1">
                    <Button
                      size="sm"
                      variant="outline"
                      className="text-xs justify-start gap-1.5"
                      onClick={() => onSelectLayout?.("dashboard")}
                    >
                      <Layers className="h-3.5 w-3.5" />
                      Dashboard Preview
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      className="text-xs justify-start gap-1.5"
                      onClick={() => onSelectLayout?.("auth")}
                    >
                      <ShieldCheck className="h-3.5 w-3.5" />
                      Auth Preview
                    </Button>
                  </div>
                </CardContent>
                <CardFooter className="bg-muted/30 py-2 px-4 text-[11px] text-muted-foreground border-t">
                  MarketingLayout, DashboardLayout, AuthLayout の3種類を標準搭載
                </CardFooter>
              </Card>
            )}
          </>
        )}
      </div>
    </section>
  )
}
