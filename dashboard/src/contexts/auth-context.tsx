"use client";

import * as React from "react";
import {
  UserIdentity,
  getCurrentUser,
  loginWithPasskey as clientLoginWithPasskey,
  getGoogleLoginUrl,
  sendEmailOTP,
  verifyEmailOTP,
  logout as clientLogout,
} from "@/lib/auth-client";

interface AuthContextValue {
  user: UserIdentity | null;
  isLoading: boolean;
  loginWithPasskey: (useConditionalUI?: boolean) => Promise<void>;
  loginWithGoogle: () => Promise<void>;
  sendOTP: (email: string) => Promise<void>;
  verifyOTP: (email: string, code: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshUser: () => Promise<void>;
}

const AuthContext = React.createContext<AuthContextValue | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = React.useState<UserIdentity | null>(null);
  const [isLoading, setIsLoading] = React.useState(true);

  const refreshUser = React.useCallback(async () => {
    try {
      const u = await getCurrentUser();
      setUser(u);
    } catch {
      setUser(null);
    } finally {
      setIsLoading(false);
    }
  }, []);

  React.useEffect(() => {
    let isMounted = true;
    getCurrentUser()
      .then((u) => {
        if (isMounted) setUser(u);
      })
      .catch(() => {
        if (isMounted) setUser(null);
      })
      .finally(() => {
        if (isMounted) setIsLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, []);

  const loginWithPasskey = async (useConditionalUI = false) => {
    const res = await clientLoginWithPasskey(useConditionalUI);
    setUser(res.identity);
  };

  const loginWithGoogle = async () => {
    const url = await getGoogleLoginUrl();
    window.location.href = url;
  };

  const sendOTP = async (email: string) => {
    await sendEmailOTP(email);
  };

  const verifyOTP = async (email: string, code: string) => {
    const res = await verifyEmailOTP(email, code);
    setUser(res.identity);
  };

  const logout = async () => {
    await clientLogout();
    setUser(null);
  };

  return (
    <AuthContext.Provider
      value={{
        user,
        isLoading,
        loginWithPasskey,
        loginWithGoogle,
        sendOTP,
        verifyOTP,
        logout,
        refreshUser,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = React.useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
}
