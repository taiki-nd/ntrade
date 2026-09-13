"use client"

import * as React from "react"
import Link from "next/link"
import { usePathname, useRouter } from "next/navigation"
import {
  LayoutDashboard,
  FolderKanban,
  CircleDot,
  Users,
  CreditCard,
  KeyRound,
  Settings,
  Bell,
  TrendingUp,
  BrainCircuit,
  History,
  Lightbulb,
  LineChart,
  Bot,
  Menu,
  FileText,
  Search,
  PanelLeftClose,
  PanelLeftOpen,
  LogOut,
} from "lucide-react"

import { cn } from "@/lib/utils"
import { useAuth } from "@/contexts/auth-context"
import { Button, buttonVariants } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Sheet, SheetContent, SheetTrigger } from "@/components/ui/sheet"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { ThemeToggle } from "@/components/theme-toggle"
import { ColorThemeToggle } from "@/components/color-theme-toggle"

export interface NavItem {
  title: string
  href: string
  icon: React.ComponentType<{ className?: string }>
  badge?: string
}

const defaultNavItems: NavItem[] = [
  { title: "ダッシュボード", href: "/", icon: LayoutDashboard },
  { title: "LLM 思考ログ", href: "/#cot", icon: BrainCircuit, badge: "AI" },
  { title: "チャートプレビュー", href: "/#chart", icon: LineChart },
  { title: "約定履歴", href: "/#trades", icon: History },
  { title: "教訓マネージャー", href: "/#lessons", icon: Lightbulb },
  { title: "システム設定", href: "/settings", icon: Settings },
]

interface DashboardLayoutProps {
  children: React.ReactNode
  navItems?: NavItem[]
  title?: string
}

interface DashboardNavContentProps {
  navItems: NavItem[]
  pathname: string
  isCollapsed?: boolean
  onItemClick?: () => void
  onToggleCollapse?: () => void
}

function DashboardNavContent({
  navItems,
  pathname,
  isCollapsed = false,
  onItemClick,
  onToggleCollapse,
}: DashboardNavContentProps) {
  const { user } = useAuth()
  const userInitial = user?.email ? user.email.charAt(0).toUpperCase() : "OP"
  const userEmail = user?.email || "Local Operator"
  const userRole = user?.role || "Trader"

  return (
    <div className="flex h-full flex-col justify-between py-3">
      <div className="space-y-4">
        {/* Header / Logo */}
        <div
          className={cn(
            "flex items-center h-10 px-3",
            isCollapsed ? "justify-center" : "justify-between"
          )}
        >
          {isCollapsed ? (
            <button
              type="button"
              onClick={onToggleCollapse}
              className="flex h-8 w-8 items-center justify-center rounded-lg bg-emerald-600 text-white font-bold text-xs cursor-pointer hover:opacity-90 transition-opacity"
              title="ntrade (クリックしてサイドバーを展開)"
            >
              nt
            </button>
          ) : (
            <>
              <Link href="/" className="flex items-center gap-2.5 overflow-hidden">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-emerald-600 text-white font-bold text-xs">
                  nt
                </div>
                <span className="font-bold text-base tracking-tight truncate">
                  ntrade
                </span>
                <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded text-muted-foreground font-mono">
                  v0.1
                </span>
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
                  <span className="sr-only">Toggle sidebar</span>
                </Button>
              )}
            </>
          )}
        </div>

        {/* Nav Items */}
        <div className={cn(isCollapsed ? "px-2" : "px-3")}>
          {!isCollapsed && (
            <p className="px-3 pb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Menu
            </p>
          )}
          <nav className="space-y-1">
            {navItems.map((item) => {
              const Icon = item.icon
              const isActive = pathname === item.href

              if (isCollapsed) {
                return (
                  <Link
                    key={item.href}
                    href={item.href}
                    onClick={onItemClick}
                    title={item.title + (item.badge ? ` (${item.badge})` : "")}
                    className={cn(
                      "flex h-10 w-10 mx-auto items-center justify-center rounded-lg transition-colors",
                      isActive
                        ? "bg-accent text-accent-foreground font-semibold"
                        : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                    )}
                  >
                    <Icon className="h-4 w-4 shrink-0" />
                    <span className="sr-only">{item.title}</span>
                  </Link>
                )
              }

              return (
                <Link
                  key={item.href}
                  href={item.href}
                  onClick={onItemClick}
                  className={cn(
                    "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                    isActive
                      ? "bg-accent text-accent-foreground font-semibold"
                      : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                  )}
                >
                  <Icon className="h-4 w-4 shrink-0" />
                  <span className="truncate">{item.title}</span>
                  {item.badge && (
                    <span className="ml-auto rounded-full bg-primary/10 px-2 py-0.5 text-xs text-primary">
                      {item.badge}
                    </span>
                  )}
                </Link>
              )
            })}
          </nav>
        </div>
      </div>

      {/* Footer User Profile */}
      <div className={cn("border-t pt-3", isCollapsed ? "px-2 flex justify-center" : "px-4")}>
        {isCollapsed ? (
          <div className="cursor-pointer py-1" title={`${userEmail}${userRole ? ` (${userRole})` : ""}`}>
            <Avatar className="h-8 w-8">
              <AvatarFallback>{userInitial}</AvatarFallback>
            </Avatar>
          </div>
        ) : (
          <div className="flex items-center gap-3">
            <Avatar className="h-8 w-8 shrink-0">
              <AvatarFallback>{userInitial}</AvatarFallback>
            </Avatar>
            <div className="flex-1 overflow-hidden">
              <p className="truncate text-sm font-medium">{userEmail}</p>
              <p className="truncate text-xs text-muted-foreground capitalize">
                {userRole || (user ? "User" : "Not signed in")}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

export function DashboardLayout({
  children,
  navItems = defaultNavItems,
  title = "App Dashboard",
}: DashboardLayoutProps) {
  const pathname = usePathname()
  const router = useRouter()
  const { user, logout } = useAuth()
  const [open, setOpen] = React.useState(false)
  const [sidebarCollapsed, setSidebarCollapsed] = React.useState(false)
  const [isScrolled, setIsScrolled] = React.useState(false)

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const scrollTop = e.currentTarget.scrollTop
    setIsScrolled(scrollTop > 10)
  }

  const handleLogout = async () => {
    await logout()
    router.push("/login")
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
          isCollapsed={sidebarCollapsed}
          onToggleCollapse={() => setSidebarCollapsed((prev) => !prev)}
        />
      </aside>

      {/* Main Content Area (一体化スクロールコンテナ) */}
      <div
        className="flex flex-1 flex-col h-full overflow-y-auto min-w-0"
        onScroll={handleScroll}
      >
        {/* Top Header */}
        <header
          className={cn(
            "sticky top-0 z-30 flex h-14 shrink-0 items-center justify-between px-4 transition-all duration-300 md:px-6",
            isScrolled
              ? "border-b border-border/50 bg-background/80 backdrop-blur-md shadow-xs"
              : "border-b border-transparent bg-transparent"
          )}
        >
          <div className="flex items-center gap-2 sm:gap-3">
            {/* Mobile Nav Trigger */}
            <Sheet open={open} onOpenChange={setOpen}>
              <SheetTrigger
                className={cn(
                  buttonVariants({ variant: "ghost", size: "icon" }),
                  "md:hidden cursor-pointer"
                )}
              >
                <Menu className="h-5 w-5" />
                <span className="sr-only">Toggle navigation</span>
              </SheetTrigger>
              <SheetContent side="left" className="w-64 p-0">
                <DashboardNavContent
                  navItems={navItems}
                  pathname={pathname}
                  isCollapsed={false}
                  onItemClick={() => setOpen(false)}
                />
              </SheetContent>
            </Sheet>

            {/* Desktop Sidebar Toggle Button: Show ONLY when collapsed */}
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

            <h1 className="text-base font-semibold md:text-lg">{title}</h1>
          </div>

          <div className="flex items-center gap-3">
            <div className="relative hidden sm:block w-48 lg:w-64">
              <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
              <Input
                type="search"
                placeholder="Search..."
                className="pl-8 h-9 bg-muted/50 text-sm"
              />
            </div>
            <Button variant="ghost" size="icon" className="relative h-9 w-9">
              <Bell className="h-4 w-4" />
              <span className="absolute top-2 right-2 h-2 w-2 rounded-full bg-primary" />
              <span className="sr-only">Notifications</span>
            </Button>
            <ColorThemeToggle />
            <ThemeToggle />
            <DropdownMenu>
              <DropdownMenuTrigger className="relative h-9 w-9 rounded-full cursor-pointer outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring">
                <Avatar className="h-8 w-8">
                  <AvatarFallback>
                    {user?.email ? user.email.charAt(0).toUpperCase() : "U"}
                  </AvatarFallback>
                </Avatar>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-56">
                <DropdownMenuGroup>
                  <DropdownMenuLabel>
                    <div className="flex flex-col space-y-1">
                      <p className="text-sm font-medium leading-none truncate">
                        {user?.email || "My Account"}
                      </p>
                      {user?.role && (
                        <p className="text-xs leading-none text-muted-foreground capitalize">
                          Role: {user.role}
                        </p>
                      )}
                    </div>
                  </DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem
                    onClick={() => router.push("/settings")}
                    className="cursor-pointer"
                  >
                    Settings
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  {user ? (
                    <DropdownMenuItem
                      onClick={handleLogout}
                      className="text-destructive focus:text-destructive cursor-pointer"
                    >
                      <LogOut className="mr-2 h-4 w-4" />
                      Log out
                    </DropdownMenuItem>
                  ) : (
                    <DropdownMenuItem
                      onClick={() => router.push("/login")}
                      className="cursor-pointer font-medium text-primary"
                    >
                      Sign In
                    </DropdownMenuItem>
                  )}
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </header>

        {/* Page Content */}
        <main className="flex-1 p-4 md:p-6 lg:p-8">
          {children}
        </main>
      </div>
    </div>
  )
}
