import { startAuthentication, startRegistration } from '@simplewebauthn/browser';
import { setAuthToken } from '@/generated/api/client';

const API_BASE_URL = typeof window === 'undefined' ? (process.env.API_BASE_URL || 'http://localhost:8080') : '';

export const TOKEN_KEY = 'bee8_auth_token';
export const DEV_DEFAULT_TOKEN = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIwMU0yQVI5QThEWFdDMjc2SjRKS1RDTkpURyIsImVtYWlsIjoidGVzdHVzZXJAYmVlOC5kZXYiLCJyb2xlIjoidXNlciIsImV4cCI6MTc5MTg3MDk4MSwiaWF0IjoxNzg5Mjc4OTgxfQ.MSedVurfFeZS7IgpzWqqj12rTjNu1f4fFMSSLusW_bU';

export function getStoredToken(): string | null {
  if (typeof window === 'undefined') return null;
  let token = localStorage.getItem(TOKEN_KEY);
  if (!token && window.location.hostname === 'localhost') {
    token = DEV_DEFAULT_TOKEN;
    localStorage.setItem(TOKEN_KEY, token);
    document.cookie = `token=${token}; path=/; max-age=2592000; SameSite=Lax`;
  }
  if (token) {
    setAuthToken(token);
  }
  return token;
}

export function setStoredToken(token: string | null): void {
  setAuthToken(token);
  if (typeof window === 'undefined') return;
  if (token) {
    localStorage.setItem(TOKEN_KEY, token);
    document.cookie = `token=${token}; path=/; max-age=2592000; SameSite=Lax`;
  } else {
    localStorage.removeItem(TOKEN_KEY);
    document.cookie = 'token=; path=/; max-age=0; SameSite=Lax';
  }
}

export interface UserIdentity {
  id: string;
  email: string;
  role: string;
}

export interface PasskeyDevice {
  id: string;
  name: string;
  created_at: string;
  last_used_at?: string;
}

export async function authFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const token = getStoredToken();
  const headers = new Headers(options?.headers);
  headers.set('Content-Type', 'application/json');
  if (token) {
    headers.set('Authorization', `Bearer ${token}`);
  }

  const res = await fetch(`${API_BASE_URL}${path}`, {
    ...options,
    headers,
  });

  if (!res.ok) {
    let errMsg = `Request failed: ${res.statusText}`;
    try {
      const body = await res.json();
      if (body.error?.message) errMsg = body.error.message;
    } catch {}
    throw new Error(errMsg);
  }

  return (await res.json()) as T;
}

// Passkeys
export async function loginWithPasskey(useConditionalUI = false): Promise<{ token: string; identity: UserIdentity }> {
  const beginRes = await authFetch<{
    data: { session_id: string; options: Parameters<typeof startAuthentication>[0]['optionsJSON'] };
  }>('/auth/passkey/login/begin', {
    method: 'POST',
  });
  const { session_id, options } = beginRes.data;

  const authResponse = await startAuthentication({
    optionsJSON: options,
    useBrowserAutofill: useConditionalUI,
  });

  const finishRes = await authFetch<{ data: { token: string; identity: UserIdentity } }>('/auth/passkey/login/finish', {
    method: 'POST',
    headers: { 'X-Passkey-Session-ID': session_id },
    body: JSON.stringify(authResponse),
  });

  setStoredToken(finishRes.data.token);
  return finishRes.data;
}

export async function registerPasskey(name: string): Promise<void> {
  const beginRes = await authFetch<{
    data: { session_id: string; options: Parameters<typeof startRegistration>[0]['optionsJSON'] };
  }>('/auth/passkey/register/begin', {
    method: 'POST',
  });
  const { session_id, options } = beginRes.data;

  const regResponse = await startRegistration({ optionsJSON: options });

  await authFetch(`/auth/passkey/register/finish?name=${encodeURIComponent(name)}`, {
    method: 'POST',
    headers: { 'X-Passkey-Session-ID': session_id },
    body: JSON.stringify(regResponse),
  });
}

export async function listPasskeys(): Promise<PasskeyDevice[]> {
  const res = await authFetch<{ data: PasskeyDevice[] }>('/auth/passkeys');
  return res.data;
}

export async function deletePasskey(id: string): Promise<void> {
  await authFetch(`/auth/passkeys/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
}

// Google OAuth
export async function getGoogleLoginUrl(): Promise<string> {
  const res = await authFetch<{ data: { url: string } }>('/auth/google/login');
  return res.data.url;
}

export async function handleGoogleCallback(code: string): Promise<{ token: string; identity: UserIdentity }> {
  const res = await authFetch<{ data: { token: string; identity: UserIdentity } }>('/auth/google/callback', {
    method: 'POST',
    body: JSON.stringify({ code }),
  });
  setStoredToken(res.data.token);
  return res.data;
}

// Email OTP
export async function sendEmailOTP(email: string): Promise<void> {
  await authFetch('/auth/otp/send', {
    method: 'POST',
    body: JSON.stringify({ email }),
  });
}

export async function verifyEmailOTP(email: string, code: string): Promise<{ token: string; identity: UserIdentity }> {
  const res = await authFetch<{ data: { token: string; identity: UserIdentity } }>('/auth/otp/signin', {
    method: 'POST',
    body: JSON.stringify({ email, code }),
  });
  if (res.data?.token) {
    setStoredToken(res.data.token);
  }
  return res.data;
}

// Session
export async function getCurrentUser(): Promise<UserIdentity | null> {
  const token = getStoredToken();
  if (!token) return null;
  try {
    const res = await authFetch<{ data: { identity: UserIdentity } }>('/auth/me');
    return res.data.identity;
  } catch {
    setStoredToken(null);
    return null;
  }
}

export async function logout(): Promise<void> {
  try {
    await authFetch('/auth/logout', { method: 'POST' });
  } finally {
    setStoredToken(null);
  }
}
