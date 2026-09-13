"use client"

import * as React from "react"
import { useRouter } from "next/navigation"

import { MarketingLayout } from "@/components/layouts/marketing-layout"
import { ComponentCatalog } from "@/components/component-catalog"

export default function ComponentsPage() {
  const router = useRouter()

  const handleSelectLayout = (layout: "showcase" | "dashboard" | "auth") => {
    if (layout === "dashboard") {
      router.push("/dashboard")
    } else if (layout === "auth") {
      router.push("/login")
    } else {
      router.push("/")
    }
  }

  return (
    <MarketingLayout>
      <div className="container mx-auto max-w-5xl px-4 py-8 sm:py-12">
        <ComponentCatalog onSelectLayout={handleSelectLayout} />
      </div>
    </MarketingLayout>
  )
}
