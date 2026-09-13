"use client";

import * as React from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { toast } from "sonner";
import { Loader2 } from "lucide-react";
import { handleGoogleCallback } from "@/lib/auth-client";
import { useAuth } from "@/contexts/auth-context";

function CallbackContent() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const { refreshUser } = useAuth();
  const [error, setError] = React.useState<string | null>(null);
  const code = searchParams.get("code");

  React.useEffect(() => {
    if (!code) return;

    let isMounted = true;
    handleGoogleCallback(code)
      .then(async () => {
        if (!isMounted) return;
        await refreshUser();
        toast.success("Googleアカウントでログインしました");
        router.push("/dashboard");
      })
      .catch((err: unknown) => {
        if (!isMounted) return;
        const msg = err instanceof Error ? err.message : "ログイン処理に失敗しました";
        setError(msg);
      });

    return () => {
      isMounted = false;
    };
  }, [code, router, refreshUser]);

  const displayError = error || (!code ? "認可コードが見つかりませんでした" : null);

  if (displayError) {
    return (
      <div className="min-h-screen flex items-center justify-center p-4">
        <div className="max-w-md w-full p-6 text-center space-y-4 border rounded-xl bg-card shadow-xs">
          <p className="text-destructive font-medium">{displayError}</p>
          <button
            onClick={() => router.push("/login")}
            className="text-sm text-primary underline cursor-pointer"
          >
            ログイン画面に戻る
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex flex-col items-center justify-center space-y-4">
      <Loader2 className="h-8 w-8 animate-spin text-primary" />
      <p className="text-sm text-muted-foreground">認証処理中...</p>
    </div>
  );
}

export default function AuthCallbackPage() {
  return (
    <React.Suspense
      fallback={
        <div className="min-h-screen flex flex-col items-center justify-center space-y-4">
          <Loader2 className="h-8 w-8 animate-spin text-primary" />
          <p className="text-sm text-muted-foreground">読み込み中...</p>
        </div>
      }
    >
      <CallbackContent />
    </React.Suspense>
  );
}
