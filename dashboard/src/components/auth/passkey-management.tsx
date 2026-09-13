"use client";

import * as React from "react";
import { toast } from "sonner";
import { Fingerprint, Plus, Trash2, Loader2, Smartphone, Laptop } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { PasskeyDevice, listPasskeys, registerPasskey, deletePasskey } from "@/lib/auth-client";

export function PasskeyManagement() {
  const [passkeys, setPasskeys] = React.useState<PasskeyDevice[]>([]);
  const [isLoading, setIsLoading] = React.useState(true);
  const [isRegistering, setIsRegistering] = React.useState(false);

  const fetchPasskeys = React.useCallback(async () => {
    try {
      const data = await listPasskeys();
      setPasskeys(data || []);
    } catch {
      // Ignore if unauthenticated or endpoint not yet configured
    } finally {
      setIsLoading(false);
    }
  }, []);

  React.useEffect(() => {
    let isMounted = true;
    listPasskeys()
      .then((data) => {
        if (isMounted) setPasskeys(data || []);
      })
      .catch(() => {})
      .finally(() => {
        if (isMounted) setIsLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, []);

  const handleRegister = async () => {
    setIsRegistering(true);
    try {
      // Default device name by user agent
      const isMobile = /iPhone|iPad|Android/i.test(navigator.userAgent);
      const defaultName = isMobile ? "スマートフォン (Touch ID / Face ID)" : "PC端末 (生体認証)";
      const name = prompt("この端末の名前を入力してください:", defaultName) || defaultName;

      await registerPasskey(name);
      toast.success("パスキーを登録しました");
      await fetchPasskeys();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "パスキーの登録に失敗しました";
      toast.error(msg);
    } finally {
      setIsRegistering(false);
    }
  };

  const handleDelete = async (id: string, name: string) => {
    if (!confirm(`パスキー「${name}」を削除してもよろしいですか？`)) {
      return;
    }

    try {
      await deletePasskey(id);
      toast.success("パスキーを削除しました");
      setPasskeys((prev) => prev.filter((p) => p.id !== id));
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "削除に失敗しました";
      toast.error(msg);
    }
  };

  return (
    <Card className="shadow-xs">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div className="space-y-1">
            <CardTitle className="text-base flex items-center gap-2">
              <Fingerprint className="h-5 w-5 text-primary" />
              パスキー管理
            </CardTitle>
            <CardDescription>
              Touch ID、Face ID、Windows Hello などの生体認証器を登録して 1 タップで安全にログインできます。
            </CardDescription>
          </div>
          <Button
            size="sm"
            onClick={handleRegister}
            disabled={isRegistering}
            className="gap-1.5"
          >
            {isRegistering ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Plus className="h-4 w-4" />
            )}
            この端末を登録
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <div className="py-6 flex justify-center">
            <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
          </div>
        ) : passkeys.length === 0 ? (
          <div className="py-8 text-center border border-dashed rounded-lg text-sm text-muted-foreground">
            登録されたパスキーはありません。「この端末を登録」をクリックして生体認証を追加してください。
          </div>
        ) : (
          <div className="divide-y border rounded-lg">
            {passkeys.map((pk) => (
              <div
                key={pk.id}
                className="p-3.5 flex items-center justify-between hover:bg-muted/50 transition-colors"
              >
                <div className="flex items-center gap-3">
                  <div className="h-9 w-9 rounded-full bg-primary/10 flex items-center justify-center text-primary">
                    {pk.name.includes("スマートフォン") || pk.name.includes("iPhone") ? (
                      <Smartphone className="h-4 w-4" />
                    ) : (
                      <Laptop className="h-4 w-4" />
                    )}
                  </div>
                  <div>
                    <p className="text-sm font-medium">{pk.name}</p>
                    <p className="text-xs text-muted-foreground">
                      登録日: {new Date(pk.created_at).toLocaleDateString()}
                      {pk.last_used_at && ` • 最終使用: ${new Date(pk.last_used_at).toLocaleDateString()}`}
                    </p>
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => handleDelete(pk.id, pk.name)}
                  className="text-muted-foreground hover:text-destructive h-8 w-8 p-0"
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
