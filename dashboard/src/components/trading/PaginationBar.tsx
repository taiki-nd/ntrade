"use client";

import * as React from "react";
import {
  Pagination,
  PaginationContent,
  PaginationEllipsis,
  PaginationItem,
  PaginationLink,
  PaginationNext,
  PaginationPrevious,
} from "@/components/ui/pagination";
import { cn } from "@/lib/utils";

export interface PaginationBarProps {
  /** 1 始まり */
  page: number;
  pageSize: number;
  total: number;
  onPageChange: (page: number) => void;
}

/** 現在ページ周辺のページ番号を返す（離れた箇所は "ellipsis"） */
function getPageItems(current: number, total: number): (number | "ellipsis")[] {
  if (total <= 7) return Array.from({ length: total }, (_, i) => i + 1);
  const items: (number | "ellipsis")[] = [1];
  const start = Math.max(2, current - 1);
  const end = Math.min(total - 1, current + 1);
  if (start > 2) items.push("ellipsis");
  for (let p = start; p <= end; p++) items.push(p);
  if (end < total - 1) items.push("ellipsis");
  items.push(total);
  return items;
}

/** 件数表示とページ送り。1ページに収まる場合は件数のみ表示 */
export function PaginationBar({ page, pageSize, total, onPageChange }: PaginationBarProps) {
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const current = Math.min(page, totalPages);
  if (total === 0) return null;

  const goTo = (p: number) => (e: React.MouseEvent) => {
    e.preventDefault();
    onPageChange(Math.min(Math.max(1, p), totalPages));
  };

  return (
    <div className="flex flex-col sm:flex-row items-center justify-between gap-2">
      <span className="text-xs text-muted-foreground">
        全 {total} 件中 {(current - 1) * pageSize + 1}–{Math.min(current * pageSize, total)} 件
      </span>
      {totalPages > 1 && (
        <Pagination className="mx-0 w-auto">
          <PaginationContent>
            <PaginationItem>
              <PaginationPrevious
                onClick={goTo(current - 1)}
                aria-disabled={current === 1}
                className={cn(current === 1 && "pointer-events-none opacity-50")}
              />
            </PaginationItem>
            {getPageItems(current, totalPages).map((item, i) =>
              item === "ellipsis" ? (
                <PaginationItem key={`e${i}`}>
                  <PaginationEllipsis />
                </PaginationItem>
              ) : (
                <PaginationItem key={item}>
                  <PaginationLink isActive={item === current} onClick={goTo(item)}>
                    {item}
                  </PaginationLink>
                </PaginationItem>
              )
            )}
            <PaginationItem>
              <PaginationNext
                onClick={goTo(current + 1)}
                aria-disabled={current === totalPages}
                className={cn(current === totalPages && "pointer-events-none opacity-50")}
              />
            </PaginationItem>
          </PaginationContent>
        </Pagination>
      )}
    </div>
  );
}
