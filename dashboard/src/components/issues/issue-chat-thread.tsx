"use client"

import * as React from "react"
import { IssueComment } from "@/types/schema"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Badge } from "@/components/ui/badge"
import { Send, Bot, User, Sparkles, CheckCheck, Loader2 } from "lucide-react"

interface IssueChatThreadProps {
  issueId: string
  initialComments: IssueComment[]
  onExecuteFix?: () => void
  onSendMessage?: (msg: string) => Promise<string>
  onTriggerFix?: () => Promise<any>
}

export function IssueChatThread({
  issueId,
  initialComments,
  onExecuteFix,
  onSendMessage,
  onTriggerFix,
}: IssueChatThreadProps) {
  const [comments, setComments] = React.useState<IssueComment[]>(initialComments)
  const [inputText, setInputText] = React.useState("")
  const [isFixing, setIsFixing] = React.useState(false)
  const [isSending, setIsSending] = React.useState(false)
  const [fixDone, setFixDone] = React.useState(false)

  // Sync with initialComments when updated from parent
  React.useEffect(() => {
    if (initialComments && initialComments.length > 0) {
      setComments(initialComments)
    }
  }, [initialComments])

  const handleSend = async () => {
    if (!inputText.trim() || isSending) return

    const msg = inputText.trim()
    const userComment: IssueComment = {
      id: `cmt_${Date.now()}`,
      issue_id: issueId,
      sender_type: "user",
      sender_name: "あなた",
      content: msg,
      created_at: new Date().toISOString(),
    }

    setComments((prev) => [...prev, userComment])
    setInputText("")
    setIsSending(true)

    try {
      if (onSendMessage) {
        const reply = await onSendMessage(msg)
        const aiResponse: IssueComment = {
          id: `cmt_${Date.now() + 1}`,
          issue_id: issueId,
          sender_type: "ai",
          sender_name: "bee8 AI Agent",
          content: reply,
          created_at: new Date().toISOString(),
        }
        setComments((prev) => [...prev, aiResponse])
      } else {
        // AIの自動応答シミュレーション
        setTimeout(() => {
          const aiResponse: IssueComment = {
            id: `cmt_${Date.now() + 1}`,
            issue_id: issueId,
            sender_type: "ai",
            sender_name: "bee8 AI Agent",
            content:
              "承知しました。要件を反映して仕様を更新しました。「修正を実行」ボタンを押すと自律コード生成とプレビュー反映が始まります。",
            metadata: {
              action_type: "fix_triggered",
            },
            created_at: new Date().toISOString(),
          }
          setComments((prev) => [...prev, aiResponse])
        }, 1000)
      }
    } finally {
      setIsSending(false)
    }
  }

  const handleExecuteFix = async () => {
    setIsFixing(true)
    try {
      if (onTriggerFix) {
        const result = await onTriggerFix()
        setFixDone(true)
        const systemComment: IssueComment = {
          id: `cmt_${Date.now()}`,
          issue_id: issueId,
          sender_type: "system",
          sender_name: "bee8 Pipeline",
          content: result?.summary || "自律修正完了: bee8 lint すべて合格。プレビュー環境を更新しました。",
          metadata: {
            action_type: "preview_deployed",
          },
          created_at: new Date().toISOString(),
        }
        setComments((prev) => [...prev, systemComment])
        onExecuteFix?.()
      } else {
        setTimeout(() => {
          setIsFixing(false)
          setFixDone(true)
          const systemComment: IssueComment = {
            id: `cmt_${Date.now()}`,
            issue_id: issueId,
            sender_type: "system",
            sender_name: "bee8 Pipeline",
            content:
              "自律修正完了: bee8 lint（txcheck / querycheck / authcheck）すべて合格。プレビュー環境を更新しました。",
            metadata: {
              action_type: "preview_deployed",
            },
            created_at: new Date().toISOString(),
          }
          setComments((prev) => [...prev, systemComment])
          onExecuteFix?.()
        }, 2500)
      }
    } finally {
      setIsFixing(false)
    }
  }

  return (
    <Card className="flex flex-col h-[680px]">
      {/* スレッドヘッダー */}
      <CardHeader className="py-3 px-4 border-b border-border flex flex-row items-center justify-between space-y-0">
        <div className="flex items-center gap-2">
          <Bot className="h-4 w-4 text-primary" />
          <CardTitle className="text-sm font-semibold">
            AI 要件壁打ちスレッド
          </CardTitle>
        </div>
        <Button
          size="sm"
          className="gap-1.5 h-8 text-xs"
          disabled={isFixing || fixDone}
          onClick={handleExecuteFix}
        >
          {isFixing ? (
            <>
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              自律修正中...
            </>
          ) : fixDone ? (
            <>
              <CheckCheck className="h-3.5 w-3.5" />
              修正済み
            </>
          ) : (
            <>
              <Sparkles className="h-3.5 w-3.5" />
              修正を実行
            </>
          )}
        </Button>
      </CardHeader>

      {/* メッセージスクロール領域 */}
      <CardContent className="flex-1 overflow-y-auto p-4 space-y-4">
        {comments.map((comment) => {
          const isAI = comment.sender_type === "ai"
          const isSystem = comment.sender_type === "system"

          if (isSystem) {
            return (
              <div
                key={comment.id}
                className="p-2.5 rounded-lg bg-muted/60 border border-border text-center text-xs text-muted-foreground flex items-center justify-center gap-1.5"
              >
                <Sparkles className="h-3.5 w-3.5 text-foreground" />
                {comment.content}
              </div>
            )
          }

          return (
            <div
              key={comment.id}
              className={`flex gap-3 ${isAI ? "items-start" : "items-start flex-row-reverse"}`}
            >
              <Avatar className="h-7 w-7 mt-0.5 border border-border">
                <AvatarFallback className={isAI ? "bg-muted text-foreground text-xs" : "bg-primary text-primary-foreground text-xs"}>
                  {isAI ? <Bot className="h-4 w-4" /> : <User className="h-4 w-4" />}
                </AvatarFallback>
              </Avatar>

              <div className={`space-y-1 max-w-[85%] ${isAI ? "" : "text-right"}`}>
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <span className="font-medium text-foreground">
                    {comment.sender_name || (isAI ? "AI Agent" : "ユーザー")}
                  </span>
                  <span>
                    {new Date(comment.created_at).toLocaleTimeString("ja-JP", {
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </span>
                </div>

                <div
                  className={`p-3 rounded-xl text-xs sm:text-sm leading-relaxed whitespace-pre-wrap ${
                    isAI
                      ? "bg-muted/40 border border-border text-foreground"
                      : "bg-primary text-primary-foreground ml-auto text-left"
                  }`}
                >
                  {comment.content}

                  {typeof comment.metadata === "object" && comment.metadata?.token_usage && (
                    <div className="mt-2 pt-2 border-t border-border/50 text-[10px] text-muted-foreground flex items-center gap-1">
                      <Sparkles className="h-2.5 w-2.5" />
                      消費トークン: {comment.metadata.token_usage}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )
        })}
      </CardContent>

      {/* 入力フォーム */}
      <div className="p-3 border-t border-border bg-card/50 space-y-2">
        <Textarea
          placeholder="要件の変更や追加の要望を入力..."
          value={inputText}
          onChange={(e) => setInputText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault()
              handleSend()
            }
          }}
          className="min-h-[70px] max-h-[140px] text-sm resize-none"
        />
        <div className="flex items-center justify-between">
          <span className="text-[11px] text-muted-foreground">
            Enter で送信 / Shift + Enter で改行
          </span>
          <Button
            size="sm"
            onClick={handleSend}
            disabled={!inputText.trim()}
            className="gap-1.5 h-8 px-3"
          >
            送信
            <Send className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>
    </Card>
  )
}
