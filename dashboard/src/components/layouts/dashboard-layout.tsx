"use client"

import * as React from "react"
import Link from "next/link"
import { usePathname } from "next/navigation"
import {
  LayoutDashboard,
  BrainCircuit,
  LineChart,
  History,
  Lightbulb,
  FlaskConical,
  Settings,
  Menu,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react"

import { cn } from "@/lib/utils"
import { useHash, setHash } from "@/hooks/use-hash"
import { Button, buttonVariants } from "@/components/ui/button"
import { Sheet, SheetContent, SheetTrigger } from "@/components/ui/sheet"
import { ThemeToggle } from "@/components/theme-toggle"
import { ColorThemeToggle } from "@/components/color-theme-toggle"

export interface NavItem {
  title: string
  href: string
  icon: React.ComponentType<{ className?: string }>
  badge?: string
}

/** ダッシュボードのタブは `/#<tab>` で表現する。page.tsx がハッシュを読んでタブを切り替える。 */
const defaultNavItems: NavItem[] = [
  { title: "全体概要", href: "/#overview", icon: LayoutDashboard },
  { title: "LLM 思考ログ", href: "/#cot", icon: BrainCircuit, badge: "AI" },
  { title: "チャート", href: "/#chart", icon: LineChart },
  { title: "約定履歴", href: "/#trades", icon: History },
  { title: "教訓ルール", href: "/#lessons", icon: Lightbulb },
  { title: "リプレイ", href: "/#replay", icon: FlaskConical },
  { title: "システム設定", href: "/settings", icon: Settings },
]

interface DashboardLayoutProps {
  children: React.ReactNode
  navItems?: NavItem[]
  title?: string
  /** 指定するとヘッダーのタイトルを置き換え、常時表示のステータスバーとして描画する */
  headerContent?: React.ReactNode
}

interface DashboardNavContentProps {
  navItems: NavItem[]
  pathname: string
  hash: string
  isCollapsed?: boolean
  onItemClick?: () => void
  onToggleCollapse?: () => void
}

function isItemActive(item: NavItem, pathname: string, hash: string): boolean {
  const [path, itemHash] = item.href.split("#")
  if (path !== pathname) return false
  if (itemHash === undefined) return true
  return (hash || "overview") === itemHash
}

function DashboardNavContent({
  navItems,
  pathname,
  hash,
  isCollapsed = false,
  onItemClick,
  onToggleCollapse,
}: DashboardNavContentProps) {
  return (
    <div className="flex h-full flex-col justify-between py-3">
      <div className="space-y-4">
        {/* Header / Logo */}
        <div className={cn("flex items-center h-10 px-3", isCollapsed ? "justify-center" : "justify-between")}>
          {isCollapsed ? (
            <button
              type="button"
              onClick={onToggleCollapse}
              className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground font-bold text-xs cursor-pointer hover:opacity-90 transition-opacity"
              title="ntrade (クリックしてサイドバーを展開)"
            >
              nt
            </button>
          ) : (
            <>
              <Link href="/" className="flex items-center gap-2.5 overflow-hidden">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground font-bold text-xs">
                  nt
                </div>
                <span className="font-bold text-base tracking-tight truncate">ntrade</span>
                <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded text-muted-foreground font-mono">v0.1</span>
              </Link>
              {onToggleCollapse && (
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7 text-muted-foreground hover:text-foreground hidden md:flex cursor-pointer shrink-0"
                  onClick={onToggleCollapse}
                  title="サイドバーを縮小"
                >
                  <PanelLeftClose className="h-4 w-4" />
                  <span className="sr-only">サイドバーを折りたたむ</span>
                </Button>
              )}
            </>
          )}
        </div>

        {/* Nav Items */}
        <div className={cn(isCollapsed ? "px-2" : "px-3")}>
          {!isCollapsed && (
            <p className="px-3 pb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Menu</p>
          )}
          <nav className="space-y-1">
            {navItems.map((item) => {
              const Icon = item.icon
              const isActive = isItemActive(item, pathname, hash)
              // 同一ページ内のハッシュ遷移は Link (pushState) だと hashchange が発火せず、
              // useHash が次の再描画（5秒ポーリング）まで追従しない。setHash で即時通知する。
              const [itemPath, itemHash] = item.href.split("#")
              const handleClick = (e: React.MouseEvent<HTMLAnchorElement>) => {
                if (itemHash !== undefined && itemPath === pathname) {
                  e.preventDefault()
                  setHash(itemHash)
                }
                onItemClick?.()
              }

              if (isCollapsed) {
                return (
                  <Link
                    key={item.href}
                    href={item.href}
                    onClick={handleClick}
                    title={item.title + (item.badge ? ` (${item.badge})` : "")}
                    className={cn(
                      "flex h-10 w-10 mx-auto items-center justify-center rounded-lg transition-colors",
                      isActive ? "bg-accent text-accent-foreground" : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                    )}
                  >
                    <Icon className="h-5 w-5" />
                    <span className="sr-only">{item.title}</span>
                  </Link>
                )
              }

              return (
                <Link
                  key={item.href}
                  href={item.href}
                  onClick={handleClick}
                  className={cn(
                    "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                    isActive ? "bg-accent text-accent-foreground" : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                  )}
                >
                  <Icon className="h-4 w-4 shrink-0" />
                  <span className="truncate">{item.title}</span>
                  {item.badge && (
                    <span className="ml-auto text-[10px] bg-muted px-1.5 py-0.5 rounded text-muted-foreground font-mono">
                      {item.badge}
                    </span>
                  )}
                </Link>
              )
            })}
          </nav>
        </div>
      </div>

      {/* Footer */}
      {!isCollapsed && (
        <div className="border-t pt-3 px-4">
          <p className="text-xs text-muted-foreground">ローカル運用 / Rust エンジン :4000</p>
        </div>
      )}
    </div>
  )
}

export function DashboardLayout({ children, navItems = defaultNavItems, title = "ntrade", headerContent }: DashboardLayoutProps) {
  const pathname = usePathname()
  const hash = useHash()
  const [open, setOpen] = React.useState(false)
  const [sidebarCollapsed, setSidebarCollapsed] = React.useState(false)
  const [isScrolled, setIsScrolled] = React.useState(false)

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    setIsScrolled(e.currentTarget.scrollTop > 10)
  }

  return (
    <div className="flex h-screen h-dvh w-full overflow-hidden bg-muted/30">
      {/* Desktop Sidebar (Collapsible: w-64 <-> w-16) */}
      <aside
        className={cn(
          "hidden border-r bg-background md:block h-full sticky top-0 overflow-y-auto overflow-x-hidden shrink-0 transition-[width] duration-300 ease-in-out z-20",
          sidebarCollapsed ? "w-16" : "w-64"
        )}
      >
        <DashboardNavContent
          navItems={navItems}
          pathname={pathname}
          hash={hash}
          isCollapsed={sidebarCollapsed}
          onToggleCollapse={() => setSidebarCollapsed((prev) => !prev)}
        />
      </aside>

      {/* Main Content Area */}
      <div className="flex flex-1 flex-col h-full overflow-y-auto min-w-0" onScroll={handleScroll}>
        <header
          className={cn(
            "sticky top-0 z-30 flex min-h-14 shrink-0 items-center justify-between gap-3 px-4 transition-all duration-300 md:px-6",
            headerContent
              ? "border-b border-border bg-background/90 backdrop-blur-md"
              : isScrolled
                ? "border-b border-border/50 bg-background/80 backdrop-blur-md shadow-xs"
                : "border-b border-transparent bg-transparent"
          )}
        >
          <div className={cn("flex items-center gap-2 sm:gap-3", headerContent && "min-w-0 flex-1")}>
            <Sheet open={open} onOpenChange={setOpen}>
              <SheetTrigger className={cn(buttonVariants({ variant: "ghost", size: "icon" }), "md:hidden cursor-pointer")}>
                <Menu className="h-5 w-5" />
                <span className="sr-only">ナビゲーションを開閉</span>
              </SheetTrigger>
              <SheetContent side="left" className="w-64 p-0">
                <DashboardNavContent
                  navItems={navItems}
                  pathname={pathname}
                  hash={hash}
                  isCollapsed={false}
                  onItemClick={() => setOpen(false)}
                />
              </SheetContent>
            </Sheet>

            {sidebarCollapsed && (
              <Button
                variant="ghost"
                size="icon"
                className="hidden md:flex cursor-pointer text-muted-foreground hover:text-foreground"
                onClick={() => setSidebarCollapsed(false)}
                title="サイドバーを展開"
              >
                <PanelLeftOpen className="h-5 w-5" />
                <span className="sr-only">サイドバーを展開</span>
              </Button>
            )}

            {headerContent ? (
              <>
                <h1 className="sr-only">{title}</h1>
                {headerContent}
              </>
            ) : (
              <h1 className="text-base font-semibold md:text-lg">{title}</h1>
            )}
          </div>

          <div className="flex items-center gap-2 shrink-0">
            <ColorThemeToggle />
            <ThemeToggle />
          </div>
        </header>

        <main className="flex-1 p-4 md:p-5 lg:p-6">{children}</main>

      </div>
    </div>
  )
}
