/**
 * api/schema に準拠した TypeScript 型定義
 * バックエンド (bee8 generate) が出力するクライアント DTO と 1:1 で互換
 */

export type DatabaseType = "postgres" | "mysql" | "sqlite"
export type ProjectStatus = "active" | "archived"

export interface Project {
  id: string
  identity_id?: string
  organization_id?: string
  name: string
  slug: string
  database_type: DatabaseType
  status: ProjectStatus
  preview_url?: string
  repo_url?: string
  setup_prompt?: string
  created_at: string
  updated_at?: string
}

export type IssueStatus = "open" | "chatting" | "fixing" | "review_ready" | "closed"
export type IssuePriority = "low" | "medium" | "high" | "urgent"

export interface Issue {
  id: string
  project_id: string
  created_by_id?: string
  title: string
  description?: string
  status: IssueStatus
  priority: IssuePriority
  branch_name?: string
  preview_url?: string
  diff_summary?: {
    files_changed: number
    additions: number
    deletions: number
    summary: string
  }
  created_at: string
  updated_at?: string
  closed_at?: string
}

export type CommentSenderType = "user" | "ai" | "system"

export interface CommentMetadata {
  diff_patch?: string
  action_type?: "fix_triggered" | "lint_passed" | "preview_deployed"
  token_usage?: number
}

export interface IssueComment {
  id: string
  issue_id: string
  sender_id?: string
  sender_name?: string
  sender_type: CommentSenderType
  content: string
  metadata?: string | CommentMetadata
  created_at: string
}

export type TriggerType = "manual" | "issue_fix" | "setup" | "ci"
export type GenerationStatus = "success" | "failed"

export interface GenerationLog {
  id: string
  project_id: string
  issue_id?: string
  trigger_type: TriggerType
  status: GenerationStatus
  lint_passed: boolean
  error_code?: string
  duration_ms: number
  domain_count: number
  route_count: number
  output_bytes: number
  token_count: number
  created_at: string
}

export interface Organization {
  id: string
  name: string
  slug: string
  created_at: string
  updated_at: string
}

export type OrgRole = "owner" | "member" | "client"

export interface Membership {
  id: string
  user_id: string
  user_name: string
  user_email: string
  organization_id: string
  role: OrgRole
  created_at: string
}

export type PlanType = "free" | "pro" | "team" | "enterprise"
export type SubscriptionStatus = "active" | "past_due" | "canceled" | "trialing"

export interface Subscription {
  id: string
  plan: PlanType
  status: SubscriptionStatus
  current_period_end?: string
  cancel_at_period_end: boolean
  monthly_generations_used: number
  monthly_generations_limit: number
  projects_used: number
  projects_limit: number
}

export type LicenseStatus = "active" | "revoked" | "expired"

export interface LicenseKey {
  id: string
  subscription_id: string
  key_prefix: string
  key_masked: string
  status: LicenseStatus
  expires_at: string
  last_verified_at?: string
  created_at: string
}
