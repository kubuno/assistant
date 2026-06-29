import { create } from 'zustand'
import { jarvisApi, ConversationSummary, JarvisMessage, JarvisToolCall, JarvisAgent, JarvisFolder, ModelInfo } from './api'

interface JarvisState {
  conversations:   ConversationSummary[]
  folders:         JarvisFolder[]
  activeConvId:    string | null
  messages:        JarvisMessage[]
  agents:          JarvisAgent[]
  models:          ModelInfo[]
  selectedModel:    string | null
  selectedProvider: string | null
  selectedAgentId:  string | null
  isStreaming:     boolean
  streamingText:   string
  streamingToolCalls: JarvisToolCall[]
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
  appendToolCall:      (call: JarvisToolCall) => void
  finalizeStream:      (msgId: string, promptTokens: number, completionTokens: number) => void
  startStream:         () => void
  addUserMessage:      (content: string) => void
  streamChat:          (convId: string, body: { content?: string; regenerate?: boolean }) => Promise<void>
  stopStream:          () => void
  regenerate:          (convId: string) => Promise<void>
  setMessageFeedback:  (convId: string, msgId: string, feedback: 'like' | 'dislike') => Promise<void>
  togglePin:           (id: string) => Promise<void>
  renameConversation:  (id: string, title: string) => Promise<void>
  deleteConversation:  (id: string) => Promise<void>
  archiveConversation: (id: string) => Promise<void>
  reorderConversations: (orderedIds: string[]) => Promise<void>
  deleteMessage:       (convId: string, msgId: string) => Promise<void>
  editUserMessage:     (convId: string, msgId: string, content: string) => Promise<void>
  createConversation:  (opts?: { title?: string; agentId?: string }) => Promise<string>
  // Projets (dossiers de conversations)
  fetchFolders:        () => Promise<void>
  createFolder:        (name: string) => Promise<string | undefined>
  renameFolder:        (id: string, name: string) => Promise<void>
  deleteFolder:        (id: string) => Promise<void>
  moveConversation:    (convId: string, folderId: string | null) => Promise<void>
}

// Contrôleur d'annulation du flux courant (hors état zustand pour éviter les rerenders).
let streamAbort: AbortController | null = null

export const useJarvisStore = create<JarvisState>((set, get) => ({
  conversations:   [],
  folders:         [],
  activeConvId:    null,
  messages:        [],
  agents:          [],
  models:          [],
  selectedModel:    null,
  selectedProvider: null,
  selectedAgentId:  null,
  isStreaming:     false,
  streamingText:   '',
  streamingToolCalls: [],
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

  startStream: () => set({ isStreaming: true, streamingText: '', streamingToolCalls: [] }),

  appendDelta: (delta) => set(s => ({ streamingText: s.streamingText + delta })),

  appendToolCall: (call) => set(s => ({ streamingToolCalls: [...s.streamingToolCalls, call] })),

  finalizeStream: (msgId, promptTokens, completionTokens) => {
    const content = get().streamingText
    const toolCalls = get().streamingToolCalls
    const activeConvId = get().activeConvId
    const assistantMsg: JarvisMessage = {
      id:                msgId,
      conversation_id:   activeConvId ?? '',
      role:              'assistant',
      content,
      tool_calls:        toolCalls.length ? toolCalls : undefined,
      prompt_tokens:     promptTokens,
      completion_tokens: completionTokens,
      created_at:        new Date().toISOString(),
    }
    set(s => ({
      isStreaming:        false,
      streamingText:      '',
      streamingToolCalls: [],
      messages:           [...s.messages, assistantMsg],
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

  // Flux SSE centralisé : envoie un message (ou régénère) et applique les deltas /
  // tool calls / finalisation. Utilisé par la zone de saisie ET la régénération.
  streamChat: async (convId, body) => {
    get().startStream()
    const ctrl = new AbortController()
    streamAbort = ctrl
    try {
      const sdk = await import('@kubuno/sdk')
      const token = sdk.useAuthStore.getState().accessToken
      const resp = await fetch(`/api/v1/jarvis/conversations/${convId}/chat`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
        body: JSON.stringify({ content: body.content ?? '', model: get().selectedModel ?? undefined, regenerate: body.regenerate ?? false }),
        signal: ctrl.signal,
      })
      if (!resp.ok || !resp.body) { get().finalizeStream(crypto.randomUUID(), 0, 0); return }
      const reader = resp.body.getReader()
      const decoder = new TextDecoder()
      let buf = ''
      for (;;) {
        const { done, value: chunk } = await reader.read()
        if (done) break
        buf += decoder.decode(chunk, { stream: true })
        const lines = buf.split('\n')
        buf = lines.pop() ?? ''
        for (const line of lines) {
          if (!line.startsWith('data:')) continue
          const data = line.slice(5).trim()
          if (data === '[DONE]' || !data) continue
          try {
            const evt = JSON.parse(data)
            if (evt.type === 'delta') get().appendDelta(evt.content)
            else if (evt.type === 'tool_call') {
              const call = evt.call as JarvisToolCall
              get().appendToolCall(call)
              if (call.kind === 'ui' && call.ui) {
                try { sdk.ModuleServiceRegistry.call(call.ui.service, call.ui.method, call.args) }
                catch (e) { console.error('dispatch action UI', e) }
              }
            }
            else if (evt.type === 'done') get().finalizeStream(evt.message_id, evt.prompt_tokens, evt.completion_tokens)
            else if (evt.type === 'error') get().appendDelta(`\n\n⚠️ ${evt.message}`)
          } catch { /* ignore */ }
        }
      }
    } catch (err: unknown) {
      if (err instanceof Error && err.name !== 'AbortError') console.error('SSE error', err)
      if (get().isStreaming) get().finalizeStream(crypto.randomUUID(), 0, 0)
    } finally {
      streamAbort = null
    }
  },

  stopStream: () => { streamAbort?.abort() },

  // Régénère la dernière réponse : retire le dernier message assistant de l'UI puis
  // relance le tour (le backend supprime aussi sa version persistée).
  regenerate: async (convId) => {
    if (get().isStreaming) return
    set(s => {
      const msgs = [...s.messages]
      while (msgs.length && msgs[msgs.length - 1].role === 'assistant') msgs.pop()
      return { messages: msgs }
    })
    await get().streamChat(convId, { regenerate: true })
  },

  setMessageFeedback: async (convId, msgId, feedback) => {
    // Bascule : recliquer sur le même retour le retire.
    const cur = get().messages.find(m => m.id === msgId)?.feedback
    const next = cur === feedback ? null : feedback
    set(s => ({ messages: s.messages.map(m => m.id === msgId ? { ...m, feedback: next } : m) }))
    try { await jarvisApi.setFeedback(convId, msgId, next) }
    catch { set(s => ({ messages: s.messages.map(m => m.id === msgId ? { ...m, feedback: cur ?? null } : m) })) }
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

  renameConversation: async (id, title) => {
    const t = title.trim()
    if (!t) return
    const updated = await jarvisApi.updateConversation(id, { title: t })
    set(s => ({ conversations: s.conversations.map(c => c.conversation.id === id ? { ...c, conversation: updated } : c) }))
  },

  deleteConversation: async (id) => {
    await jarvisApi.deleteConversation(id)
    set(s => ({
      conversations: s.conversations.filter(c => c.conversation.id !== id),
      activeConvId:  s.activeConvId === id ? null : s.activeConvId,
      messages:      s.activeConvId === id ? [] : s.messages,
    }))
  },

  // Archivage : la liste exclut les conversations archivées → on la retire localement.
  archiveConversation: async (id) => {
    const prev = get().conversations
    set(s => ({
      conversations: s.conversations.filter(c => c.conversation.id !== id),
      activeConvId:  s.activeConvId === id ? null : s.activeConvId,
      messages:      s.activeConvId === id ? [] : s.messages,
    }))
    try { await jarvisApi.updateConversation(id, { is_archived: true }) }
    catch { set({ conversations: prev }) }
  },

  // Réordonne (glisser-déposer) : `orderedIds` = nouvel ordre d'une section.
  // Position = index dans la section ; mise à jour optimiste + persistance.
  reorderConversations: async (orderedIds) => {
    const posById = new Map(orderedIds.map((id, i) => [id, i]))
    set(s => ({
      conversations: s.conversations.map(c =>
        posById.has(c.conversation.id)
          ? { ...c, conversation: { ...c.conversation, position: posById.get(c.conversation.id)! } }
          : c),
    }))
    try {
      await Promise.all(orderedIds.map((id, i) =>
        jarvisApi.updateConversation(id, { position: i })))
    } catch { get().fetchConversations() } // resync si échec
  },

  deleteMessage: async (convId, msgId) => {
    set(s => ({ messages: s.messages.filter(m => m.id !== msgId) }))
    try { await jarvisApi.deleteMessage(convId, msgId) }
    catch { get().fetchMessages(convId) } // resync si échec
  },

  // Modifier un message utilisateur : on retire ce message ET tout ce qui suit
  // (comme ChatGPT — la suite est invalidée), puis on renvoie le texte modifié.
  editUserMessage: async (convId, msgId, content) => {
    const c = content.trim()
    if (!c || get().isStreaming) return
    const msgs = get().messages
    const idx = msgs.findIndex(m => m.id === msgId)
    if (idx < 0) return
    const removed = msgs.slice(idx)
    set({ messages: msgs.slice(0, idx) })
    for (const m of removed) { try { await jarvisApi.deleteMessage(convId, m.id) } catch { /* déjà absent */ } }
    get().addUserMessage(c)
    await get().streamChat(convId, { content: c })
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

  // ── Dossiers ──────────────────────────────────────────────────────────────────
  fetchFolders: async () => {
    const data = await jarvisApi.listFolders()
    set({ folders: data })
  },

  createFolder: async (name) => {
    const n = name.trim()
    if (!n) return undefined
    const f = await jarvisApi.createFolder({ name: n })
    set(s => ({ folders: [...s.folders, f] }))
    return f.id
  },

  renameFolder: async (id, name) => {
    const n = name.trim()
    if (!n) return
    const f = await jarvisApi.updateFolder(id, { name: n })
    set(s => ({ folders: s.folders.map(x => x.id === id ? f : x) }))
  },

  deleteFolder: async (id) => {
    await jarvisApi.deleteFolder(id)
    // Les conversations du dossier sont détachées (folder_id → null) côté serveur.
    set(s => ({
      folders: s.folders.filter(f => f.id !== id),
      conversations: s.conversations.map(c =>
        c.conversation.folder_id === id ? { ...c, conversation: { ...c.conversation, folder_id: null } } : c),
    }))
  },

  moveConversation: async (convId, folderId) => {
    set(s => ({ conversations: s.conversations.map(c =>
      c.conversation.id === convId ? { ...c, conversation: { ...c.conversation, folder_id: folderId } } : c) }))
    try { await jarvisApi.updateConversation(convId, { folder_id: folderId }) }
    catch { get().fetchConversations() }
  },
}))
