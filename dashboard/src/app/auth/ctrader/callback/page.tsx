"use client";

import * as React from "react";
import { Suspense } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { tradingApi, AccountInfo } from "@/lib/trading-api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { CheckCircle2, XCircle, Loader2, Server, ArrowRight } from "lucide-react";

function CTraderOAuthCallbackContent() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const code = searchParams.get("code");
  const errorParam = searchParams.get("error");
  const errorDesc = searchParams.get("error_description");

  const [status, setStatus] = React.useState<"loading" | "success" | "error">(
    "loading"
  );
  const [errorMessage, setErrorMessage] = React.useState<string>("");
  const [accounts, setAccounts] = React.useState<AccountInfo[]>([]);
  const [countdown, setCountdown] = React.useState<number>(3);

  React.useEffect(() => {
    if (errorParam) {
      setStatus("error");
      setErrorMessage(errorDesc || errorParam);
      return;
    }

    if (!code) {
      setStatus("error");
      setErrorMessage("認可コード (code) が URL パラメータに見つかりませんでした。");
      return;
    }

    let isMounted = true;

    async function exchange() {
      try {
        const res = await tradingApi.exchangeOAuthCode(code!);
        if (!isMounted) return;

        if (res.success) {
          setStatus("success");
          setAccounts(res.data || []);
        } else {
          setStatus("error");
          setErrorMessage(res.message || "トークンの交換に失敗しました。");
        }
      } catch (err: unknown) {
        if (!isMounted) return;
        setStatus("error");
        setErrorMessage(
          err instanceof Error
            ? err.message
            : "Rustコアエンジン (localhost:4000) への接続に失敗しました。"
        );
      }
    }

    exchange();

    return () => {
      isMounted = false;
    };
  }, [code, errorParam, errorDesc]);

  // 成功時の自動カウントダウンリダイレクト
  React.useEffect(() => {
    if (status !== "success") return;

    if (countdown <= 0) {
      router.push("/");
      return;
    }

    const timer = setTimeout(() => {
      setCountdown((prev) => prev - 1);
    }, 1000);

    return () => clearTimeout(timer);
  }, [status, countdown, router]);

  return (
    <Card className="w-full max-w-md shadow-lg border">
      <CardHeader className="text-center">
        <div className="mx-auto mb-3 flex h-14 w-14 items-center justify-center rounded-full">
          {status === "loading" && (
            <div className="h-12 w-12 rounded-full bg-primary/10 flex items-center justify-center">
              <Loader2 className="h-6 w-6 animate-spin text-primary" />
            </div>
          )}
          {status === "success" && (
            <div className="h-12 w-12 rounded-full bg-emerald-500/10 flex items-center justify-center">
              <CheckCircle2 className="h-8 w-8 text-emerald-500" />
            </div>
          )}
          {status === "error" && (
            <div className="h-12 w-12 rounded-full bg-destructive/10 flex items-center justify-center">
              <XCircle className="h-8 w-8 text-destructive" />
            </div>
          )}
        </div>

        <CardTitle className="text-xl">
          {status === "loading" && "cTrader アカウントを認証中..."}
          {status === "success" && "cTrader 連携が完了しました！"}
          {status === "error" && "連携に失敗しました"}
        </CardTitle>

        <CardDescription>
          {status === "loading" &&
            "Spotware 認可サーバーと安全にトークンを交換し、設定を同期しています。"}
          {status === "success" &&
            "Access Token / Refresh Token の保存および口座接続が完了しました。"}
          {status === "error" && "エラーが発生したため、認証を完了できませんでした。"}
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        {status === "loading" && (
          <div className="space-y-2 text-center text-sm text-muted-foreground">
            <p>・ 認可コードの検証中...</p>
            <p>・ トークンを .env に自動保存中...</p>
            <p>・ cTrader Open API ソケットに接続中...</p>
          </div>
        )}

        {status === "success" && (
          <div className="rounded-lg border bg-card p-3 space-y-2">
            <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
              <Server className="h-3.5 w-3.5 text-emerald-500" />
              連携完了口座:
            </div>
            {accounts.length > 0 ? (
              accounts.map((acc) => (
                <div
                  key={acc.ctidTraderAccountId}
                  className="flex items-center justify-between text-sm py-1 border-b last:border-0"
                >
                  <div>
                    <span className="font-semibold">#{acc.traderLogin}</span>
                    <span className="ml-2 text-xs text-muted-foreground">
                      {acc.brokerTitle}
                    </span>
                  </div>
                  <span
                    className={`text-xs px-2 py-0.5 rounded font-medium ${
                      acc.isLive
                        ? "bg-rose-500/10 text-rose-600"
                        : "bg-blue-500/10 text-blue-600"
                    }`}
                  >
                    {acc.isLive ? "LIVE" : "DEMO"}
                  </span>
                </div>
              ))
            ) : (
              <p className="text-xs text-muted-foreground">
                口座情報が自動設定されました。
              </p>
            )}
          </div>
        )}

        {status === "error" && (
          <div className="rounded-md bg-destructive/10 p-3 text-xs text-destructive">
            <p className="font-semibold mb-1">エラー詳細:</p>
            <p>{errorMessage}</p>
          </div>
        )}
      </CardContent>

      <CardFooter className="flex flex-col gap-2">
        {status === "success" && (
          <Button
            className="w-full gap-2 cursor-pointer bg-emerald-600 hover:bg-emerald-700"
            onClick={() => router.push("/")}
          >
            ダッシュボードへ戻る ({countdown}s)
            <ArrowRight className="h-4 w-4" />
          </Button>
        )}

        {status === "error" && (
          <Button
            variant="outline"
            className="w-full cursor-pointer"
            onClick={() => router.push("/")}
          >
            ダッシュボードへ戻る
          </Button>
        )}
      </CardFooter>
    </Card>
  );
}

export default function CTraderOAuthCallbackPage() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-muted/30 p-4">
      <Suspense
        fallback={
          <div className="flex items-center justify-center p-8">
            <Loader2 className="h-8 w-8 animate-spin text-primary" />
          </div>
        }
      >
        <CTraderOAuthCallbackContent />
      </Suspense>
    </div>
  );
}
