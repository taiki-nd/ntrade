# Frontend Development Rules

## UI Components
- Always import UI components from `@/components/ui/*`. Never reinvent custom buttons, inputs, tables, or modals using raw Tailwind/HTML.
- Use pre-built layouts from `@/components/layouts/*` (DashboardLayout, MarketingLayout, AuthLayout).

## Design Constraints
- Do NOT use heavy linear gradients (e.g. purple/blue/pink AI gradients).
- Use semantic tokens: `bg-background`, `text-foreground`, `text-muted-foreground`, `border-border`.
- Use `lucide-react` for all icons.
- Avoid unnecessary animations or oversized box-shadows. Keep it clean, minimal, and professional.
