import { api as apiClient } from '@kubuno/sdk'

export interface AssistantConversation {
  id:            string
  user_id:       string
  agent_id:      string | null
  title:         string | null
  model:         string
  message_count: number
  total_tokens:  number
  is_pinned:     boolean
  is_archived:   boolean
  folder_id:     string | null
  position:      number
  created_at:    string
  updated_at:    string
}

export interface ConversationSummary {
  conversation: AssistantConversation
  last_message: string | null
}

export interface AssistantFolder {
  id:         string
  owner_id:   string
  name:       string
  color:      string | null
  position:   number
  created_at: string
  updated_at: string
}

/** One tool the assistant invoked (or a client UI action / pending confirmation). */
export interface AssistantToolCall {
  tool:      string
  kind:      'backend' | 'ui' | 'confirm'
  args:      Record<string, unknown>
  result?:   string
  is_error?: boolean
  ui?:       { service: string; method: string }
}

export interface AssistantMessage {
  id:              string
  conversation_id: string
  role:            'user' | 'assistant' | 'system'
  content:         string
  tool_calls?:     AssistantToolCall[]
  prompt_tokens:   number
  completion_tokens: number
  feedback?:       'like' | 'dislike' | null
  created_at:      string
}

export interface AgentSuggestion { label: string; prompt: string; icon?: string }

export interface AssistantAgent {
  id:            string
  name:          string
  description:   string | null
  system_prompt: string
  default_model: string | null
  avatar_emoji?: string | null
  avatar_color?: string | null
  prompt_suggestions?: AgentSuggestion[]
  is_system:     boolean
  created_by:    string | null
  created_at:    string
  updated_at:    string
}

export interface ModelInfo {
  id:         string
  name:       string
  provider:   string
  is_default: boolean
}

export interface ProviderConfig {
  provider:      string
  enabled:       boolean
  /** Masked preview only — the backend never returns the key itself. */
  api_key:       string
  /** Whether a key is stored, so a form can say so without parsing the mask. */
  has_api_key:   boolean
  base_url:      string
  default_model: string
}

export interface UpdateProviderDto {
  enabled?:       boolean
  api_key?:       string
  base_url?:      string
  default_model?: string
}

export const assistantApi = {
  // Conversations
  listConversations: () =>
    apiClient.get<ConversationSummary[]>('/assistant/conversations').then(r => r.data),

  getConversation: (id: string) =>
    apiClient.get<AssistantConversation>(`/assistant/conversations/${id}`).then(r => r.data),

  createConversation: (data: { title?: string; agent_id?: string; model?: string; provider?: string }) =>
    apiClient.post<AssistantConversation>('/assistant/conversations', data).then(r => r.data),

  updateConversation: (id: string, data: { title?: string; is_pinned?: boolean; is_archived?: boolean; model?: string; folder_id?: string | null; position?: number }) =>
    apiClient.patch<AssistantConversation>(`/assistant/conversations/${id}`, data).then(r => r.data),

  deleteConversation: (id: string) =>
    apiClient.delete(`/assistant/conversations/${id}`),

  // Dossiers (organisation des conversations)
  listFolders: () =>
    apiClient.get<AssistantFolder[]>('/assistant/folders').then(r => r.data),
  createFolder: (data: { name: string; color?: string }) =>
    apiClient.post<AssistantFolder>('/assistant/folders', data).then(r => r.data),
  updateFolder: (id: string, data: { name?: string; color?: string; position?: number }) =>
    apiClient.patch<AssistantFolder>(`/assistant/folders/${id}`, data).then(r => r.data),
  deleteFolder: (id: string) =>
    apiClient.delete(`/assistant/folders/${id}`),

  listMessages: (id: string) =>
    apiClient.get<AssistantMessage[]>(`/assistant/conversations/${id}/messages`).then(r => r.data),

  // Retour 👍/👎 sur un message (null pour retirer).
  setFeedback: (convId: string, msgId: string, feedback: 'like' | 'dislike' | null) =>
    apiClient.patch(`/assistant/conversations/${convId}/messages/${msgId}/feedback`, { feedback }),

  // Supprime un message d'une conversation.
  deleteMessage: (convId: string, msgId: string) =>
    apiClient.delete(`/assistant/conversations/${convId}/messages/${msgId}`),

  // Execute a single tool (after the user confirms a `confirm`-gated tool).
  callTool: (tool: string, args: Record<string, unknown>) =>
    apiClient.post<{ result: string; is_error: boolean }>('/assistant/tools/call', { tool, arguments: args }).then(r => r.data),

  // Agents
  listAgents: () =>
    apiClient.get<AssistantAgent[]>('/assistant/agents').then(r => r.data),

  createAgent: (data: { name: string; description?: string; system_prompt: string; default_model?: string }) =>
    apiClient.post<AssistantAgent>('/assistant/agents', data).then(r => r.data),

  updateAgent: (id: string, data: Partial<{ name: string; description: string; system_prompt: string; default_model: string }>) =>
    apiClient.patch<AssistantAgent>(`/assistant/agents/${id}`, data).then(r => r.data),

  deleteAgent: (id: string) =>
    apiClient.delete(`/assistant/agents/${id}`),

  // Models
  listModels: () =>
    apiClient.get<ModelInfo[]>('/assistant/models').then(r => r.data),

  // Provider settings
  listProviders: () =>
    apiClient.get<ProviderConfig[]>('/assistant/settings/providers').then(r => r.data),

  updateProvider: (provider: string, data: UpdateProviderDto) =>
    apiClient.patch<ProviderConfig>(`/assistant/settings/providers/${provider}`, data).then(r => r.data),
}
