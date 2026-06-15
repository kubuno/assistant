import { create } from 'zustand'
import { jarvisApi, ConversationSummary, JarvisMessage, JarvisAgent, ModelInfo } from './api'

interface JarvisState {
  conversations:   ConversationSummary[]
  activeConvId:    string | null
  messages:        JarvisMessage[]
  agents:          JarvisAgent[]
  models:          ModelInfo[]
  selectedModel:    string | null
  selectedProvider: string | null
  selectedAgentId:  string | null
  isStreaming:     boolean
  streamingText:   string
  isSidebarOpen:   boolean

  // Actions
  fetchConversations:  () => Promise<void>
  fetchMessages:       (id: string) => Promise<void>
  fetchAgents:         () => Promise<void>
  fetchModels:         () => Promise<void>
  setActiveConv:       (id: string | null) => void
  setSelectedModel:    (model: string) => void
  setSelectedProvider: (provider: string) => void
  setSelectedAgentId:  (id: string | null) => void
  toggleSidebar:       () => void
  appendDelta:         (delta: string) => void
  finalizeStream:      (msgId: string, promptTokens: number, completionTokens: number) => void
  startStream:         () => void
  addUserMessage:      (content: string) => void
  togglePin:           (id: string) => Promise<void>
  deleteConversation:  (id: string) => Promise<void>
  createConversation:  (opts?: { title?: string; agentId?: string }) => Promise<string>
}

export const useJarvisStore = create<JarvisState>((set, get) => ({
  conversations:   [],
  activeConvId:    null,
  messages:        [],
  agents:          [],
  models:          [],
  selectedModel:    null,
  selectedProvider: null,
  selectedAgentId:  null,
  isStreaming:     false,
  streamingText:   '',
  isSidebarOpen:   true,

  fetchConversations: async () => {
    const data = await jarvisApi.listConversations()
    set({ conversations: data })
  },

  fetchMessages: async (id) => {
    const data = await jarvisApi.listMessages(id)
    set({ messages: data, streamingText: '' })
  },

  fetchAgents: async () => {
    const data = await jarvisApi.listAgents()
    set({ agents: data })
  },

  fetchModels: async () => {
    const data = await jarvisApi.listModels()
    const defaultModel = data.find(m => m.is_default)?.id ?? data[0]?.id ?? null
    const defaultProvider = data.find(m => m.is_default)?.provider ?? data[0]?.provider ?? null
    set({
      models: data,
      selectedModel:    get().selectedModel    ?? defaultModel,
      selectedProvider: get().selectedProvider ?? defaultProvider,
    })
  },

  setActiveConv: (id) => set({ activeConvId: id }),
  setSelectedModel: (model) => set({ selectedModel: model }),
  setSelectedProvider: (provider) => set({ selectedProvider: provider }),
  setSelectedAgentId: (id) => set({ selectedAgentId: id }),
  toggleSidebar: () => set(s => ({ isSidebarOpen: !s.isSidebarOpen })),

  startStream: () => set({ isStreaming: true, streamingText: '' }),

  appendDelta: (delta) => set(s => ({ streamingText: s.streamingText + delta })),

  finalizeStream: (msgId, promptTokens, completionTokens) => {
    const content = get().streamingText
    const activeConvId = get().activeConvId
    const assistantMsg: JarvisMessage = {
      id:                msgId,
      conversation_id:   activeConvId ?? '',
      role:              'assistant',
      content,
      prompt_tokens:     promptTokens,
      completion_tokens: completionTokens,
      created_at:        new Date().toISOString(),
    }
    set(s => ({
      isStreaming:   false,
      streamingText: '',
      messages:      [...s.messages, assistantMsg],
    }))
    // Update conversations list
    get().fetchConversations()
  },

  addUserMessage: (content) => {
    const msg: JarvisMessage = {
      id:                crypto.randomUUID(),
      conversation_id:   get().activeConvId ?? '',
      role:              'user',
      content,
      prompt_tokens:     0,
      completion_tokens: 0,
      created_at:        new Date().toISOString(),
    }
    set(s => ({ messages: [...s.messages, msg] }))
  },

  togglePin: async (id) => {
    const conv = get().conversations.find(c => c.conversation.id === id)
    if (!conv) return
    const updated = await jarvisApi.updateConversation(id, { is_pinned: !conv.conversation.is_pinned })
    set(s => ({
      conversations: s.conversations.map(c =>
        c.conversation.id === id ? { ...c, conversation: updated } : c
      ),
    }))
  },

  deleteConversation: async (id) => {
    await jarvisApi.deleteConversation(id)
    set(s => ({
      conversations: s.conversations.filter(c => c.conversation.id !== id),
      activeConvId:  s.activeConvId === id ? null : s.activeConvId,
      messages:      s.activeConvId === id ? [] : s.messages,
    }))
  },

  createConversation: async (opts) => {
    const agentId = opts?.agentId ?? get().selectedAgentId ?? undefined
    const conv = await jarvisApi.createConversation({
      title:    opts?.title,
      agent_id: agentId,
      model:    get().selectedModel    ?? undefined,
      provider: get().selectedProvider ?? undefined,
    })
    await get().fetchConversations()
    return conv.id
  },
}))
