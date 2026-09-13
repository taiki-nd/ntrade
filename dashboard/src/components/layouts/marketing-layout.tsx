"use client"

import * as React from "react"
import Link from "next/link"
import { usePathname } from "next/navigation"
import { ArrowRight, Sparkles, Layers, Zap, Code2 } from "lucide-react"

import { buttonVariants } from "@/components/ui/button"
import { ThemeToggle } from "@/components/theme-toggle"
import { ColorThemeToggle } from "@/components/color-theme-toggle"
import { HamburgerMenu } from "@/components/hamburger-menu"
import { useAuth } from "@/contexts/auth-context"
import { cn } from "@/lib/utils"

interface MarketingLayoutProps {
  children: React.ReactNode
}

export const marketingNavLinks = [
  { label: "Overview", href: "/", icon: Sparkles },
  { label: "Components", href: "/components", icon: Layers, badge: "31" },
  { label: "Features", href: "/features", icon: Zap },
  { label: "Prompt Guide", href: "/prompts", icon: Code2 },
]

export function MarketingLayout({ children }: MarketingLayoutProps) {
  const pathname = usePathname()
  const { user } = useAuth()

  return (
    <div className="flex min-h-screen flex-col bg-background w-full overflow-x-hidden">
      {/* Top Fixed Header */}
      <header className="sticky top-0 z-40 w-full border-b bg-background/80 backdrop-blur-md">
        <div className="w-full mx-auto flex h-16 max-w-6xl items-center justify-between px-4 sm:px-6">
          {/* Brand Logo & Desktop Navigation */}
          <div className="flex items-center gap-6">
            <Link href="/" className="flex items-center gap-2.5 font-bold text-lg tracking-tight">
              <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground text-sm font-bold shadow-xs">
                NS
              </div>
              <span>NextScaffold</span>
            </Link>

            <nav className="hidden md:flex items-center gap-6 text-sm font-medium">
              {marketingNavLinks.map((link) => {
                const isActive =
                  pathname === link.href ||
                  (link.href !== "/" && pathname?.startsWith(link.href))

                return (
                  <Link
                    key={link.label}
                    href={link.href}
                    className={cn(
                      "transition-colors hover:text-foreground inline-flex items-center",
                      isActive
                        ? "text-foreground font-semibold"
                        : "text-muted-foreground"
                    )}
                  >
                    {link.label}
                    {link.badge && (
                      <span
                        className={cn(
                          "ml-1.5 rounded-full px-1.5 py-0.5 text-[10px]",
                          isActive
                            ? "bg-primary text-primary-foreground font-bold"
                            : "bg-primary/10 text-primary"
                        )}
                      >
                        {link.badge}
                      </span>
                    )}
                  </Link>
                )
              })}
            </nav>
          </div>

          {/* Header Right Actions */}
          <div className="flex items-center gap-1.5 sm:gap-3">
            <ColorThemeToggle className="hidden sm:inline-flex" />
            <ThemeToggle className="hidden sm:inline-flex" />
            {user ? (
              <Link
                href="/dashboard"
                className={cn(
                  buttonVariants({ variant: "outline", size: "sm" }),
                  "hidden sm:inline-flex text-xs"
                )}
              >
                Dashboard
              </Link>
            ) : (
              <Link
                href="/login"
                className={cn(
                  buttonVariants({ variant: "ghost", size: "sm" }),
                  "hidden sm:inline-flex text-xs",
                  pathname === "/login" && "bg-accent text-accent-foreground font-semibold"
                )}
              >
                Sign In
              </Link>
            )}
            {user ? (
              <Link
                href="/dashboard"
                className={cn(buttonVariants({ size: "sm" }), "h-8 text-xs gap-1.5 hidden sm:inline-flex")}
              >
                Dashboard
                <ArrowRight className="h-3.5 w-3.5" />
              </Link>
            ) : (
              <Link
                href="/signup"
                className={cn(buttonVariants({ size: "sm" }), "h-8 text-xs gap-1.5 hidden sm:inline-flex")}
              >
                Get Started
                <ArrowRight className="h-3.5 w-3.5" />
              </Link>
            )}

            {/* Mobile Hamburger Menu Component */}
            <div className="md:hidden flex items-center gap-1">
              <ThemeToggle />
              <HamburgerMenu
                navItems={marketingNavLinks}
                footer={
                  <div className="flex flex-col gap-3">
                    <div className="flex items-center justify-between">
                      <span className="text-xs text-muted-foreground">Color Theme</span>
                      <ColorThemeToggle />
                    </div>
                    <div className="flex flex-col gap-2 pt-2 border-t">
                      {user ? (
                        <Link
                          href="/dashboard"
                          className={cn(buttonVariants({ variant: "outline", size: "sm" }), "w-full text-xs")}
                        >
                          Dashboard ({user.email})
                        </Link>
                      ) : (
                        <>
                          <Link
                            href="/login"
                            className={cn(buttonVariants({ variant: "outline", size: "sm" }), "w-full text-xs")}
                          >
                            Sign In
                          </Link>
                          <Link
                            href="/signup"
                            className={cn(buttonVariants({ size: "sm" }), "w-full text-xs gap-1.5")}
                          >
                            Get Started (Sign Up)
                            <ArrowRight className="h-3.5 w-3.5" />
                          </Link>
                        </>
                      )}
                    </div>
                  </div>
                }
              />
            </div>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="flex-1">{children}</main>

      {/* Footer */}
      <footer className="border-t bg-muted/20 py-10 mt-16">
        <div className="container mx-auto max-w-6xl px-4 sm:px-6 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-muted-foreground">
          <p>© {new Date().getFullYear()} NextScaffold. Built for AI-first frontend development.</p>
          <div className="flex items-center gap-6">
            <Link href="#" className="hover:underline">Privacy</Link>
            <Link href="#" className="hover:underline">Terms</Link>
            <Link href="#" className="hover:underline">GitHub</Link>
          </div>
        </div>
      </footer>
    </div>
  )
}
