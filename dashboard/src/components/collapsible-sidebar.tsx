"use client"

import * as React from "react"
import Link from "next/link"
import { PanelLeftClose, type LucideIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

export interface SidebarNavItem {
  label: string
  href: string
  icon: LucideIcon | React.ComponentType<{ className?: string }>
  badge?: string
  active?: boolean
}

export interface CollapsibleSidebarProps {
  navItems: SidebarNavItem[]
  isCollapsed?: boolean
  onToggleCollapse?: () => void
  logo?: React.ReactNode
  footer?: React.ReactNode
  className?: string
  onItemClick?: () => void
}

export function CollapsibleSidebar({
  navItems,
  isCollapsed = false,
  onToggleCollapse,
  logo,
  footer,
  className,
  onItemClick,
}: CollapsibleSidebarProps) {
  return (
    <aside
      className={cn(
        "flex flex-col justify-between border-r bg-background overflow-y-auto overflow-x-hidden shrink-0 transition-[width] duration-300 ease-in-out py-3",
        isCollapsed ? "w-16" : "w-64",
        className
      )}
    >
      <div className="space-y-4">
        {/* Header / Logo Section */}
        <div
          className={cn(
            "flex items-center h-10 px-3",
            isCollapsed ? "justify-center" : "justify-between"
          )}
        >
          {logo ? (
            logo
          ) : isCollapsed ? (
            <button
              type="button"
              onClick={onToggleCollapse}
              className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground font-semibold text-sm cursor-pointer hover:opacity-90 transition-opacity"
              title="クリックしてサイドバーを展開"
            >
              NS
            </button>
          ) : (
            <>
              <Link href="/" className="flex items-center gap-2.5 overflow-hidden">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground font-semibold text-sm">
                  NS
                </div>
                <span className="font-semibold text-base tracking-tight truncate">
                  NextScaffold
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
                  <span className="sr-only">サイドバーを縮小</span>
                </Button>
              )}
            </>
          )}
        </div>

        {/* Navigation Items */}
        <div className={cn(isCollapsed ? "px-2" : "px-3")}>
          {!isCollapsed && (
            <p className="px-3 pb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Navigation
            </p>
          )}
          <nav className="space-y-1">
            {navItems.map((item) => {
              const Icon = item.icon
              const tooltipTitle = item.label + (item.badge ? ` (${item.badge})` : "")

              if (isCollapsed) {
                return (
                  <Link
                    key={item.label}
                    href={item.href}
                    onClick={onItemClick}
                    title={tooltipTitle}
                    className={cn(
                      "flex h-10 w-10 mx-auto items-center justify-center rounded-lg transition-colors",
                      item.active
                        ? "bg-accent text-accent-foreground font-semibold"
                        : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                    )}
                  >
                    <Icon className="h-4 w-4 shrink-0" />
                    <span className="sr-only">{item.label}</span>
                  </Link>
                )
              }

              return (
                <Link
                  key={item.label}
                  href={item.href}
                  onClick={onItemClick}
                  className={cn(
                    "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                    item.active
                      ? "bg-accent text-accent-foreground font-semibold"
                      : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                  )}
                >
                  <Icon className="h-4 w-4 shrink-0" />
                  <span className="truncate">{item.label}</span>
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

      {/* Footer Section */}
      {footer && (
        <div className={cn("border-t pt-3", isCollapsed ? "px-2" : "px-4")}>
          {footer}
        </div>
      )}
    </aside>
  )
}
