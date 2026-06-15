import { api as apiClient } from '@kubuno/sdk'

export interface JarvisConversation {
  id:            string
  user_id:       string
  agent_id:      string | null
  title:         string | null
  model:         string
  message_count: number
  total_tokens:  number
  is_pinned:     boolean
  is_archived:   boolean
  created_at:    string
  updated_at:    string
}

export interface ConversationSummary {
  conversation: JarvisConversation
  last_message: string | null
}

export interface JarvisMessage {
  id:              string
  conversation_id: string
  role:            'user' | 'assistant' | 'system'
  content:         string
  prompt_tokens:   number
  completion_tokens: number
  created_at:      string
}

export interface JarvisAgent {
  id:            string
  name:          string
  description:   string | null
  system_prompt: string
  default_model: string | null
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
  api_key:       string
  base_url:      string
  default_model: string
}

export interface UpdateProviderDto {
  enabled?:       boolean
  api_key?:       string
  base_url?:      string
  default_model?: string
}

export const jarvisApi = {
  // Conversations
  listConversations: () =>
    apiClient.get<ConversationSummary[]>('/jarvis/conversations').then(r => r.data),

  getConversation: (id: string) =>
    apiClient.get<JarvisConversation>(`/jarvis/conversations/${id}`).then(r => r.data),

  createConversation: (data: { title?: string; agent_id?: string; model?: string; provider?: string }) =>
    apiClient.post<JarvisConversation>('/jarvis/conversations', data).then(r => r.data),

  updateConversation: (id: string, data: { title?: string; is_pinned?: boolean; is_archived?: boolean; model?: string }) =>
    apiClient.patch<JarvisConversation>(`/jarvis/conversations/${id}`, data).then(r => r.data),

  deleteConversation: (id: string) =>
    apiClient.delete(`/jarvis/conversations/${id}`),

  listMessages: (id: string) =>
    apiClient.get<JarvisMessage[]>(`/jarvis/conversations/${id}/messages`).then(r => r.data),

  // Agents
  listAgents: () =>
    apiClient.get<JarvisAgent[]>('/jarvis/agents').then(r => r.data),

  createAgent: (data: { name: string; description?: string; system_prompt: string; default_model?: string }) =>
    apiClient.post<JarvisAgent>('/jarvis/agents', data).then(r => r.data),

  updateAgent: (id: string, data: Partial<{ name: string; description: string; system_prompt: string; default_model: string }>) =>
    apiClient.patch<JarvisAgent>(`/jarvis/agents/${id}`, data).then(r => r.data),

  deleteAgent: (id: string) =>
    apiClient.delete(`/jarvis/agents/${id}`),

  // Models
  listModels: () =>
    apiClient.get<ModelInfo[]>('/jarvis/models').then(r => r.data),

  // Provider settings
  listProviders: () =>
    apiClient.get<ProviderConfig[]>('/jarvis/settings/providers').then(r => r.data),

  updateProvider: (provider: string, data: UpdateProviderDto) =>
    apiClient.patch<ProviderConfig>(`/jarvis/settings/providers/${provider}`, data).then(r => r.data),
}
