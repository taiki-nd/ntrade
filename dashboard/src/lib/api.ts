import * as React from 'react';
import {
  listProjects,
  getProject,
  createProject as apiCreateProject,
} from '@/generated/api/projectApi';
import {
  listIssues,
  getIssue,
  createIssue as apiCreateIssue,
  updateIssue as apiUpdateIssue,
} from '@/generated/api/issueApi';
import {
  listIssueComments,
} from '@/generated/api/issueCommentApi';
import {
  listGenerationLogs,
} from '@/generated/api/generationLogApi';
import {
  getSubscription,
} from '@/generated/api/subscriptionApi';
import {
  getLicenseKey,
} from '@/generated/api/licenseKeyApi';
import {
  listMemberships,
} from '@/generated/api/membershipApi';
import {
  setupChatAiProjects,
  chatIssues,
  executeFixIssues,
} from '@/generated/api/aiApi';
import { parseJSON } from '@/generated/api/client';
import type { PublicProject } from '@/generated/types/project';
import type { PublicIssue } from '@/generated/types/issue';
import type { PublicGenerationLog } from '@/generated/types/generationLog';
import type { PublicSubscription } from '@/generated/types/subscription';
import type { PublicLicenseKey } from '@/generated/types/licenseKey';
import type { PublicMembership } from '@/generated/types/membership';
import type { Issue, IssueStatus, IssuePriority, IssueComment, CommentSenderType } from '@/types/schema';

function ensureToken() {
  if (typeof window === 'undefined') return;
  try {
    const { getStoredToken } = require('./auth-client');
    getStoredToken();
  } catch {}
}

// ---------------------------------------------------------------------------
// Projects Hooks
// ---------------------------------------------------------------------------

export function useProjects() {
  const [projects, setProjects] = React.useState<PublicProject[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<Error | null>(null);

  const fetchProjects = React.useCallback(async () => {
    setLoading(true);
    try {
      ensureToken();
      const res = await listProjects();
      setProjects(res.data || []);
      setError(null);
    } catch (err: any) {
      console.error('Failed to fetch projects from API:', err);
      setProjects([]);
      setError(err);
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  return { projects, loading, error, isLive: !error && !loading, refetch: fetchProjects };
}

export function useProject(id: string) {
  const [project, setProject] = React.useState<PublicProject | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<Error | null>(null);

  const fetchProject = React.useCallback(async () => {
    if (!id) return;
    setLoading(true);
    try {
      ensureToken();
      const res = await getProject(id);
      setProject(res.data || null);
      setError(null);
    } catch (err: any) {
      console.error(`Failed to fetch project ${id}:`, err);
      setProject(null);
      setError(err);
    } finally {
      setLoading(false);
    }
  }, [id]);

  React.useEffect(() => {
    fetchProject();
  }, [fetchProject]);

  return { project, loading, error, refetch: fetchProject };
}

// ---------------------------------------------------------------------------
// Issues Hooks
// ---------------------------------------------------------------------------

function mapIssue(i: PublicIssue): Issue {
  return {
    id: i.id,
    project_id: i.project_id,
    created_by_id: i.created_by_id,
    title: i.title,
    description: i.description,
    status: i.status as IssueStatus,
    priority: i.priority as IssuePriority,
    preview_url: (i.preview_url as any)?.String || (typeof i.preview_url === 'string' ? i.preview_url : undefined),
    created_at: i.created_at,
    updated_at: i.updated_at,
  };
}

export function useIssues(projectId?: string) {
  const [issues, setIssues] = React.useState<Issue[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<Error | null>(null);

  const fetchIssues = React.useCallback(async () => {
    setLoading(true);
    try {
      ensureToken();
      const res = await listIssues();
      if (res.data) {
        const mapped = res.data.map(mapIssue);
        const filtered = projectId
          ? mapped.filter((i) => i.project_id === projectId)
          : mapped;
        setIssues(filtered);
      } else {
        setIssues([]);
      }
      setError(null);
    } catch (err: any) {
      console.error('Failed to fetch issues from API:', err);
      setIssues([]);
      setError(err);
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  React.useEffect(() => {
    fetchIssues();
  }, [fetchIssues]);

  return { issues, loading, error, isLive: !error && !loading, refetch: fetchIssues };
}

export function useIssue(issueId: string) {
  const [issue, setIssue] = React.useState<Issue | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<Error | null>(null);

  const fetchIssue = React.useCallback(async () => {
    if (!issueId) return;
    setLoading(true);
    try {
      ensureToken();
      const res = await getIssue(issueId);
      if (res.data) {
        setIssue(mapIssue(res.data));
      }
      setError(null);
    } catch (err: any) {
      console.error(`Failed to fetch issue ${issueId}:`, err);
      setIssue(null);
      setError(err);
    } finally {
      setLoading(false);
    }
  }, [issueId]);

  React.useEffect(() => {
    fetchIssue();
  }, [fetchIssue]);

  return { issue, setIssue, loading, error, refetch: fetchIssue };
}

// ---------------------------------------------------------------------------
// Issue Comments & AI Chat Hooks
// ---------------------------------------------------------------------------

export function useIssueComments(issueId: string) {
  const [comments, setComments] = React.useState<IssueComment[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [isSending, setIsSending] = React.useState(false);

  const fetchComments = React.useCallback(async () => {
    if (!issueId) return;
    setLoading(true);
    try {
      ensureToken();
      const res = await listIssueComments();
      if (res.data) {
        const filtered = res.data
          .filter((c) => c.issue_id === issueId)
          .map((c): IssueComment => ({
            id: c.id,
            issue_id: c.issue_id,
            sender_id: (c.sender_id as any)?.String || (typeof c.sender_id === 'string' ? c.sender_id : ''),
            sender_type: c.sender_type as CommentSenderType,
            content: c.content,
            metadata: (c.metadata as any)?.String ? JSON.parse((c.metadata as any).String) : undefined,
            created_at: c.created_at,
          }));
        setComments(filtered);
      } else {
        setComments([]);
      }
    } catch (err) {
      console.error('Failed to fetch comments:', err);
      setComments([]);
    } finally {
      setLoading(false);
    }
  }, [issueId]);

  React.useEffect(() => {
    fetchComments();
  }, [fetchComments]);

  // AI 壁打ち対話 (POST /issues/{id}/chat)
  const sendChatMessage = React.useCallback(
    async (message: string): Promise<string> => {
      setIsSending(true);
      try {
        ensureToken();
        const res = await chatIssues(issueId, {
          body: JSON.stringify({ message }),
        });
        const data = await parseJSON<{ data: { reply?: string; ai_response?: string; comment_id: string } }>(res);
        const replyText = data.data.reply || data.data.ai_response || '要件を承りました。';
        await fetchComments();
        return replyText;
      } catch (err: any) {
        console.error('Chat error:', err);
        throw err;
      } finally {
        setIsSending(false);
      }
    },
    [issueId, fetchComments]
  );

  // 自律修正実行 (POST /issues/{id}/execute-fix)
  const triggerExecuteFix = React.useCallback(async () => {
    try {
      ensureToken();
      const res = await executeFixIssues(issueId);
      const data = await parseJSON<{
        data: {
          log_id?: string;
          generation_log_id?: string;
          status: string;
          preview_url?: string;
          message?: string;
          summary?: string;
          lint_passed?: boolean;
        };
      }>(res);
      await fetchComments();
      return {
        generation_log_id: data.data.log_id || data.data.generation_log_id || 'log_generated',
        status: data.data.status || 'review_ready',
        preview_url: data.data.preview_url || '',
        summary: data.data.message || data.data.summary || '修正が完了しました。',
      };
    } catch (err: any) {
      console.error('Execute fix error:', err);
      throw err;
    }
  }, [issueId, fetchComments]);

  return {
    comments,
    loading,
    isSending,
    sendChatMessage,
    triggerExecuteFix,
    refetch: fetchComments,
  };
}

// ---------------------------------------------------------------------------
// Issue Creation Helper
// ---------------------------------------------------------------------------

export async function createIssue(params: {
  projectId: string;
  title: string;
  description: string;
  priority?: IssuePriority;
}): Promise<Issue> {
  ensureToken();

  let identityId = '01M2AR9A8DXWC276J4JKTCNJTG';
  try {
    const { getCurrentUser } = await import('./auth-client');
    const user = await getCurrentUser();
    if (user?.id) identityId = user.id;
  } catch {}

  const res = await apiCreateIssue({
    project_id: params.projectId,
    created_by_id: identityId,
    title: params.title,
    description: params.description,
    priority: params.priority || 'medium',
    status: 'open',
    branch_name: '',
    preview_url: '',
    closed_at: '',
  });

  return mapIssue(res.data);
}

// ---------------------------------------------------------------------------
// Generation Logs Hook
// ---------------------------------------------------------------------------

export function useGenerationLogs(projectId?: string) {
  const [logs, setLogs] = React.useState<PublicGenerationLog[]>([]);
  const [loading, setLoading] = React.useState(true);

  const fetchLogs = React.useCallback(async () => {
    setLoading(true);
    try {
      ensureToken();
      const res = await listGenerationLogs();
      if (res.data) {
        const filtered = projectId
          ? res.data.filter((l) => l.project_id === projectId)
          : res.data;
        setLogs(filtered);
      } else {
        setLogs([]);
      }
    } catch (err) {
      console.error('Failed to fetch generation logs:', err);
      setLogs([]);
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  React.useEffect(() => {
    fetchLogs();
  }, [fetchLogs]);

  return { logs, loading, refetch: fetchLogs };
}

// ---------------------------------------------------------------------------
// Subscriptions Hook
// ---------------------------------------------------------------------------

export function useSubscription(id?: string) {
  const [subscription, setSubscription] = React.useState<PublicSubscription | null>(null);
  const [loading, setLoading] = React.useState(false);

  const fetchSub = React.useCallback(async () => {
    if (!id) return;
    setLoading(true);
    try {
      ensureToken();
      const res = await getSubscription(id);
      setSubscription(res.data || null);
    } catch (err) {
      console.error('Failed to fetch subscription:', err);
      setSubscription(null);
    } finally {
      setLoading(false);
    }
  }, [id]);

  React.useEffect(() => {
    fetchSub();
  }, [fetchSub]);

  return { subscription, loading, refetch: fetchSub };
}

// ---------------------------------------------------------------------------
// License Keys Hook
// ---------------------------------------------------------------------------

export function useLicenseKey(id?: string) {
  const [licenseKey, setLicenseKey] = React.useState<PublicLicenseKey | null>(null);
  const [loading, setLoading] = React.useState(false);

  const fetchKey = React.useCallback(async () => {
    if (!id) return;
    setLoading(true);
    try {
      ensureToken();
      const res = await getLicenseKey(id);
      setLicenseKey(res.data || null);
    } catch (err) {
      console.error('Failed to fetch license key:', err);
      setLicenseKey(null);
    } finally {
      setLoading(false);
    }
  }, [id]);

  React.useEffect(() => {
    fetchKey();
  }, [fetchKey]);

  return { licenseKey, loading, refetch: fetchKey };
}

// ---------------------------------------------------------------------------
// Memberships Hook
// ---------------------------------------------------------------------------

export function useMemberships() {
  const [memberships, setMemberships] = React.useState<PublicMembership[]>([]);
  const [loading, setLoading] = React.useState(true);

  const fetchMembers = React.useCallback(async () => {
    setLoading(true);
    try {
      ensureToken();
      const res = await listMemberships();
      setMemberships(res.data || []);
    } catch (err) {
      console.error('Failed to fetch memberships:', err);
      setMemberships([]);
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    fetchMembers();
  }, [fetchMembers]);

  return { memberships, loading, refetch: fetchMembers };
}

// ---------------------------------------------------------------------------
// AI Setup Assistant (POST /projects/ai/setup-chat)
// ---------------------------------------------------------------------------

export async function sendSetupChat(userMessage: string, history: Array<{ role: string; content: string }>) {
  try {
    ensureToken();
    const res = await setupChatAiProjects({
      body: JSON.stringify({ message: userMessage, history }),
    });
    return await parseJSON<{
      data: {
        reply: string;
        suggested_domains?: string[];
        schema_yaml_draft?: string;
      };
    }>(res);
  } catch (err) {
    console.error('Setup chat API failed:', err);
    throw err;
  }
}
