"use client"

import * as React from "react"
import Link from "next/link"
import { usePathname } from "next/navigation"
import { Menu, type LucideIcon } from "lucide-react"

import { buttonVariants } from "@/components/ui/button"
import { Sheet, SheetContent, SheetTrigger, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { cn } from "@/lib/utils"

export interface HamburgerNavItem {
  label: string
  href: string
  icon?: LucideIcon | React.ComponentType<{ className?: string }>
  badge?: string
}

export interface HamburgerMenuProps {
  navItems?: HamburgerNavItem[]
  trigger?: React.ReactNode
  side?: "left" | "right"
  logo?: React.ReactNode
  footer?: React.ReactNode
  children?: React.ReactNode
  className?: string
}

export function HamburgerMenu({
  navItems,
  trigger,
  side = "left",
  logo,
  footer,
  children,
  className,
}: HamburgerMenuProps) {
  const [open, setOpen] = React.useState(false)
  const pathname = usePathname()

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger
        className={cn(
          buttonVariants({ variant: "ghost", size: "icon" }),
          "cursor-pointer text-muted-foreground hover:text-foreground",
          className
        )}
        title="メニューを開く"
      >
        {trigger || (
          <>
            <Menu className="h-5 w-5" />
            <span className="sr-only">Toggle navigation</span>
          </>
        )}
      </SheetTrigger>
      <SheetContent side={side} className="w-72 p-0 flex flex-col justify-between">
        <div className="p-4 space-y-4">
          <SheetHeader className="p-0 border-b pb-3">
            <SheetTitle className="text-left font-bold text-base flex items-center gap-2">
              {logo || (
                <>
                  <div className="flex h-7 w-7 items-center justify-center rounded-md bg-primary text-primary-foreground text-xs font-bold">
                    NS
                  </div>
                  <span>NextScaffold</span>
                </>
              )}
            </SheetTitle>
          </SheetHeader>

          {children || (
            <nav className="flex flex-col gap-1 pt-2">
              {navItems?.map((item) => {
                const Icon = item.icon
                const isActive =
                  pathname === item.href ||
                  (item.href !== "/" && pathname?.startsWith(item.href))

                return (
                  <Link
                    key={item.label}
                    href={item.href}
                    onClick={() => setOpen(false)}
                    className={cn(
                      "flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors",
                      isActive
                        ? "bg-accent text-accent-foreground font-semibold"
                        : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                    )}
                  >
                    {Icon && <Icon className="h-4 w-4 shrink-0" />}
                    <span>{item.label}</span>
                    {item.badge && (
                      <span
                        className={cn(
                          "ml-auto rounded-full px-2 py-0.5 text-xs",
                          isActive
                            ? "bg-primary text-primary-foreground font-semibold"
                            : "bg-primary/10 text-primary"
                        )}
                      >
                        {item.badge}
                      </span>
                    )}
                  </Link>
                )
              })}
            </nav>
          )}
        </div>

        {footer && (
          <div className="border-t p-4 bg-muted/20">
            {footer}
          </div>
        )}
      </SheetContent>
    </Sheet>
  )
}
