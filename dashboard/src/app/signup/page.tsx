"use client";

import * as React from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { ArrowLeft, Fingerprint, Mail, Loader2 } from "lucide-react";

import { AuthLayout } from "@/components/layouts/auth-layout";
import { Button, buttonVariants } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import { useAuth } from "@/contexts/auth-context";

export default function SignupPage() {
  const router = useRouter();
  const { user, loginWithPasskey, loginWithGoogle, sendOTP, verifyOTP } = useAuth();

  const [email, setEmail] = React.useState("");
  const [code, setCode] = React.useState("");
  const [step, setStep] = React.useState<"email" | "otp">("email");
  const [isLoading, setIsLoading] = React.useState(false);
  const [isPasskeyLoading, setIsPasskeyLoading] = React.useState(false);

  // If already logged in, redirect to dashboard
  React.useEffect(() => {
    if (user) {
      router.push("/dashboard");
    }
  }, [user, router]);

  const handlePasskeyClick = async () => {
    setIsPasskeyLoading(true);
    try {
      await loginWithPasskey(false);
      toast.success("パスキーで登録・ログインしました");
      router.push("/dashboard");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "パスキー認証に失敗しました";
      toast.error(msg);
    } finally {
      setIsPasskeyLoading(false);
    }
  };

  const handleGoogleClick = async () => {
    try {
      await loginWithGoogle();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Google連携の初期化に失敗しました";
      toast.error(msg);
    }
  };

  const handleSendOTP = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!email) return;

    setIsLoading(true);
    try {
      await sendOTP(email);
      setStep("otp");
      toast.success("認証コードをメールに送信しました");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "認証コードの送信に失敗しました";
      toast.error(msg);
    } finally {
      setIsLoading(false);
    }
  };

  const handleVerifyOTP = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!code) return;

    setIsLoading(true);
    try {
      await verifyOTP(email, code);
      toast.success("アカウントを登録・ログインしました");
      router.push("/dashboard");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "認証コードが正しくないか期限切れです";
      toast.error(msg);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <AuthLayout
      title="Create your account"
      description="パスワード不要。Google、メール認証コード、またはパスキーですぐに登録"
    >
      <div className="space-y-4">
        {/* ① Google Social Signup */}
        <Button
          type="button"
          variant="outline"
          className="w-full gap-2 h-11 shadow-xs font-medium cursor-pointer"
          onClick={handleGoogleClick}
          disabled={isPasskeyLoading || isLoading}
        >
          <svg className="h-4 w-4" viewBox="0 0 24 24">
            <path
              fill="#4285F4"
              d="M23.745 12.27c0-.7-.06-1.4-.19-2.07H12v4.51h6.6c-.29 1.52-1.14 2.82-2.4 3.68v3.05h3.88c2.27-2.09 3.66-5.17 3.66-9.17z"
            />
            <path
              fill="#34A853"
              d="M12 24c3.24 0 5.95-1.08 7.93-2.91l-3.88-3.05c-1.08.72-2.45 1.16-4.05 1.16-3.12 0-5.77-2.1-6.72-4.93H1.25v3.15C3.26 21.36 7.35 24 12 24z"
            />
            <path
              fill="#FBBC05"
              d="M5.28 14.27c-.25-.72-.38-1.49-.38-2.27s.13-1.55.38-2.27V6.58H1.25C.45 8.18 0 9.98 0 12s.45 3.82 1.25 5.42l4.03-3.15z"
            />
            <path
              fill="#EA4335"
              d="M12 4.75c1.77 0 3.35.61 4.6 1.8l3.42-3.42C17.95 1.19 15.24 0 12 0 7.35 0 3.26 2.64 1.25 6.58l4.03 3.15c.95-2.83 3.6-4.98 6.72-4.98z"
            />
          </svg>
          Google で登録
        </Button>

        {/* ② Passkey Signup / 1-Tap */}
        <Button
          type="button"
          variant="outline"
          className="w-full gap-2 h-11 text-sm font-medium shadow-xs cursor-pointer"
          onClick={handlePasskeyClick}
          disabled={isPasskeyLoading || isLoading}
        >
          {isPasskeyLoading ? (
            <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
          ) : (
            <Fingerprint className="h-4 w-4 text-primary" />
          )}
          パスキーで登録 (Touch ID / Face ID)
        </Button>

        <div className="relative my-4">
          <div className="absolute inset-0 flex items-center">
            <div className="w-full border-t border-border" />
          </div>
          <div className="relative flex justify-center text-xs uppercase">
            <span className="bg-card px-2 text-muted-foreground">またはメールアドレスで登録</span>
          </div>
        </div>

        {/* ③ Email OTP Flow */}
        {step === "email" ? (
          <form onSubmit={handleSendOTP} className="space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="signup-email">メールアドレス</Label>
              <Input
                id="signup-email"
                type="email"
                placeholder="name@example.com"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                autoComplete="email webauthn"
                required
                disabled={isLoading}
              />
            </div>
            <Button type="submit" className="w-full gap-2 cursor-pointer" disabled={isLoading}>
              {isLoading ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Mail className="h-4 w-4" />
              )}
              認証コードを送信
            </Button>
          </form>
        ) : (
          <form onSubmit={handleVerifyOTP} className="space-y-3">
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <Label htmlFor="signup-otp">6桁の認証コード</Label>
                <button
                  type="button"
                  onClick={() => setStep("email")}
                  className="text-xs text-muted-foreground hover:underline cursor-pointer"
                >
                  メールアドレス変更
                </button>
              </div>
              <Input
                id="signup-otp"
                type="text"
                placeholder="123456"
                value={code}
                onChange={(e) => setCode(e.target.value)}
                maxLength={6}
                className="text-center tracking-widest text-lg font-mono"
                required
                disabled={isLoading}
                autoFocus
              />
              <p className="text-xs text-muted-foreground">
                {email} 宛てに届いた確認コードを入力してください
              </p>
            </div>
            <Button type="submit" className="w-full cursor-pointer" disabled={isLoading}>
              {isLoading && <Loader2 className="h-4 w-4 animate-spin mr-2" />}
              登録を完了してダッシュボードへ
            </Button>
          </form>
        )}

        {/* Switch to Login */}
        <div className="pt-2 text-center text-xs text-muted-foreground">
          すでにアカウントをお持ちですか？{" "}
          <Link href="/login" className="font-semibold text-primary underline underline-offset-4">
            ログインはこちら
          </Link>
        </div>

        <Link
          href="/"
          className={cn(buttonVariants({ variant: "ghost" }), "w-full gap-1.5 text-xs text-muted-foreground cursor-pointer")}
        >
          <ArrowLeft className="h-3.5 w-3.5" />
          トップページへ戻る
        </Link>
      </div>
    </AuthLayout>
  );
}
