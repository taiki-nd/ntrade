"use client"

import * as React from "react"
import { Palette, Check } from "lucide-react"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { cn } from "@/lib/utils"

export type ColorTheme = "neutral" | "blue" | "emerald" | "violet" | "rose" | "orange"

export interface ColorPreset {
  id: ColorTheme
  name: string
  colorClass: string
  description: string
}

export const COLOR_PRESETS: ColorPreset[] = [
  {
    id: "neutral",
    name: "Neutral (Monochrome)",
    colorClass: "bg-zinc-900 dark:bg-zinc-100",
    description: "Apple / Vercel風の洗練されたモノトーン",
  },
  {
    id: "blue",
    name: "Blue",
    colorClass: "bg-blue-600",
    description: "Stripe風の信頼感あるクラシックブルー",
  },
  {
    id: "emerald",
    name: "Emerald",
    colorClass: "bg-emerald-600",
    description: "Supabase風の洗練されたクリーングリーン",
  },
  {
    id: "violet",
    name: "Violet",
    colorClass: "bg-violet-600",
    description: "Linear風のモダンなバイオレット",
  },
  {
    id: "rose",
    name: "Rose",
    colorClass: "bg-rose-600",
    description: "華やかで洗練されたローズピンク",
  },
  {
    id: "orange",
    name: "Orange",
    colorClass: "bg-orange-500",
    description: "エネルギッシュで温かみのあるオレンジ",
  },
]

export function ColorThemeToggle({ className }: { className?: string }) {
  const [currentColor, setCurrentColor] = React.useState<ColorTheme>("neutral")

  React.useEffect(() => {
    // Sync with DOM & localStorage after mount
    const saved = (localStorage.getItem("color-theme") as ColorTheme) || "neutral"
    if (saved) {
      if (saved === "neutral") {
        document.documentElement.removeAttribute("data-accent-color")
        document.documentElement.removeAttribute("data-color-theme")
      } else {
        document.documentElement.setAttribute("data-accent-color", saved)
        document.documentElement.setAttribute("data-color-theme", saved)
      }
      requestAnimationFrame(() => {
        setCurrentColor(saved)
      })
    }
  }, [])

  const applyColor = (theme: ColorTheme) => {
    setCurrentColor(theme)
    localStorage.setItem("color-theme", theme)
    if (theme === "neutral") {
      document.documentElement.removeAttribute("data-accent-color")
      document.documentElement.removeAttribute("data-color-theme")
    } else {
      document.documentElement.setAttribute("data-accent-color", theme)
      document.documentElement.setAttribute("data-color-theme", theme)
    }
  }

  const activePreset = COLOR_PRESETS.find((p) => p.id === currentColor) ?? COLOR_PRESETS[0]

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        className={cn(
          "inline-flex items-center gap-1.5 h-8 px-2.5 rounded-lg border border-input bg-background hover:bg-muted text-xs font-medium transition-colors cursor-pointer select-none",
          className
        )}
      >
        <span className={cn("size-2.5 rounded-full ring-1 ring-border shadow-xs", activePreset.colorClass)} />
        <span className="hidden sm:inline">{activePreset.name.split(" ")[0]}</span>
        <Palette className="size-3.5 text-muted-foreground ml-0.5" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        <DropdownMenuGroup>
          <DropdownMenuLabel className="text-xs font-semibold text-muted-foreground">
            Color Themes (ライブ切替)
          </DropdownMenuLabel>
          <DropdownMenuSeparator />
          {COLOR_PRESETS.map((preset) => (
            <DropdownMenuItem
              key={preset.id}
              onClick={() => applyColor(preset.id)}
              className="flex items-center justify-between py-1.5 text-xs cursor-pointer"
            >
              <div className="flex items-center gap-2">
                <span className={cn("size-3 rounded-full shadow-xs ring-1 ring-border shrink-0", preset.colorClass)} />
                <div>
                  <p className="font-medium leading-none">{preset.name}</p>
                  <p className="text-[10px] text-muted-foreground mt-0.5 leading-tight">{preset.description.slice(0, 16)}...</p>
                </div>
              </div>
              {currentColor === preset.id && (
                <Check className="size-3.5 text-primary shrink-0 ml-2" />
              )}
            </DropdownMenuItem>
          ))}
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
