import { useRef, useEffect, useState, KeyboardEvent } from 'react'
import { Plus, Square, Mic, ChevronDown, Check } from 'lucide-react'
import { createPortal } from 'react-dom'
import { useJarvisStore } from '../jarvisStore'

// ── Model selector dropdown ───────────────────────────────────────────────────

function ModelDropdown({ anchorEl, onClose }: {
  anchorEl: HTMLElement | null
  onClose:  () => void
}) {
  const { models, selectedModel, setSelectedModel } = useJarvisStore()

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (anchorEl && !anchorEl.contains(e.target as Node)) onClose()
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [anchorEl, onClose])

  if (!anchorEl) return null

  const rect = anchorEl.getBoundingClientRect()
  const top  = rect.bottom + 6
  const left = Math.max(8, rect.left)

  return createPortal(
    <div
      className="fixed bg-white border border-border rounded-2xl shadow-xl z-50 min-w-[240px] overflow-hidden py-1"
      style={{ top, left }}
    >
      {models.map(m => (
        <button
          key={m.id}
          onClick={() => { setSelectedModel(m.id); onClose() }}
          className="w-full text-left px-4 py-3 hover:bg-surface-1 transition-colors flex items-start gap-3"
        >
          <div className="mt-0.5 w-4 h-4 flex-shrink-0 flex items-center justify-center">
            {m.id === selectedModel && <Check size={14} className="text-primary" />}
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium text-text-primary">{m.name}</span>
              {m.is_default && (
                <span className="text-[10px] font-medium px-1.5 py-0.5 rounded-full bg-primary/10 text-primary">
                  Par défaut
                </span>
              )}
            </div>
            <p className="text-xs text-text-secondary mt-0.5 truncate">{m.provider}</p>
          </div>
        </button>
      ))}
      {!models.length && (
        <div className="px-4 py-3 text-sm text-text-secondary">
          Aucun modèle configuré
        </div>
      )}
    </div>,
    document.body
  )
}

// ── Main component ────────────────────────────────────────────────────────────

interface Props {
  convId?:       string
  onConvCreated?: (id: string) => void
  inputId?:      string
}

export default function ChatInput({ convId, onConvCreated, inputId }: Props) {
  const [value, setValue]           = useState('')
  const [modelOpen, setModelOpen]   = useState(false)
  const textareaRef                 = useRef<HTMLTextAreaElement>(null)
  const modelBtnRef                 = useRef<HTMLButtonElement>(null)

  const {
    isStreaming, selectedModel, models,
    addUserMessage, startStream, appendDelta, finalizeStream,
    createConversation,
  } = useJarvisStore()

  const abortRef = useRef<AbortController | null>(null)

  // Auto-resize textarea
  useEffect(() => {
    const ta = textareaRef.current
    if (!ta) return
    ta.style.height = 'auto'
    ta.style.height = Math.min(ta.scrollHeight, 200) + 'px'
  }, [value])

  const stop = () => abortRef.current?.abort()

  const submit = async () => {
    const content = value.trim()
    if (!content || isStreaming) return
    setValue('')

    // If no convId, create one first
    let targetConvId = convId
    if (!targetConvId) {
      targetConvId = await createConversation({ title: content.slice(0, 60) })
      onConvCreated?.(targetConvId)
    }

    addUserMessage(content)
    startStream()

    const ctrl = new AbortController()
    abortRef.current = ctrl

    try {
      const token = (await import('@kubuno/sdk')).useAuthStore.getState().accessToken
      const response = await fetch(`/api/v1/jarvis/conversations/${targetConvId}/chat`, {
        method:  'POST',
        headers: {
          'Content-Type':  'application/json',
          'Authorization': `Bearer ${token}`,
        },
        body:   JSON.stringify({ content, model: selectedModel ?? undefined }),
        signal: ctrl.signal,
      })

      if (!response.ok || !response.body) {
        useJarvisStore.getState().finalizeStream(crypto.randomUUID(), 0, 0)
        return
      }

      const reader  = response.body.getReader()
      const decoder = new TextDecoder()

      while (true) {
        const { done, value: chunk } = await reader.read()
        if (done) break
        const lines = decoder.decode(chunk).split('\n')
        for (const line of lines) {
          if (!line.startsWith('data:')) continue
          const data = line.slice(5).trim()
          if (data === '[DONE]') break
          try {
            const evt = JSON.parse(data)
            if (evt.type === 'delta')      appendDelta(evt.content)
            else if (evt.type === 'done')  finalizeStream(evt.message_id, evt.prompt_tokens, evt.completion_tokens)
          } catch { /* ignore */ }
        }
      }
    } catch (err: unknown) {
      if (err instanceof Error && err.name !== 'AbortError') console.error('SSE error', err)
      useJarvisStore.getState().finalizeStream(crypto.randomUUID(), 0, 0)
    }
  }

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submit() }
  }

  const currentModel = models.find(m => m.id === selectedModel)
  const modelLabel   = currentModel?.name ?? 'Modèle'

  return (
    <div className="px-4 py-4">
      <div className="max-w-3xl mx-auto">
        {/* Input card */}
        <div
          className="flex flex-col bg-white border border-border rounded-3xl shadow-sm
                     focus-within:border-primary/50 focus-within:shadow-md transition-all overflow-hidden"
        >
          {/* Textarea row */}
          <div className="flex items-center gap-2 px-4 pt-3 pb-1">
            <button
              className="flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center
                         text-text-secondary hover:bg-surface-2 transition-colors"
              title="Joindre un fichier"
            >
              <Plus size={18} />
            </button>
            <textarea
              id={inputId}
              ref={textareaRef}
              value={value}
              onChange={e => setValue(e.target.value)}
              onKeyDown={onKeyDown}
              placeholder="Demander à Jarvis"
              rows={1}
              disabled={isStreaming}
              className="flex-1 resize-none bg-transparent text-sm text-text-primary
                         placeholder:text-text-tertiary focus:outline-none
                         min-h-[28px] max-h-[200px] leading-7 disabled:opacity-60 py-0"
            />
          </div>

          {/* Bottom row: model selector + send */}
          <div className="flex items-center justify-between px-4 pb-3 pt-1">
            <div className="flex items-center gap-1">
              {/* Model selector */}
              <button
                ref={modelBtnRef}
                onClick={() => setModelOpen(o => !o)}
                className="flex items-center gap-1 text-xs font-medium text-text-secondary
                           hover:text-text-primary hover:bg-surface-2 rounded-full px-3 py-1.5 transition-colors"
              >
                <span className="w-1.5 h-1.5 rounded-full bg-primary flex-shrink-0" />
                {modelLabel}
                <ChevronDown size={12} className="ml-0.5" />
              </button>
            </div>

            <div className="flex items-center gap-1.5">
              {/* Mic */}
              <button
                className="w-8 h-8 rounded-full flex items-center justify-center
                           text-text-secondary hover:bg-surface-2 transition-colors"
                title="Saisie vocale"
              >
                <Mic size={16} />
              </button>

              {/* Send / Stop */}
              <button
                onClick={isStreaming ? stop : submit}
                disabled={!isStreaming && !value.trim()}
                className="w-9 h-9 rounded-full flex items-center justify-center transition-all
                           disabled:opacity-40 disabled:cursor-not-allowed
                           bg-primary hover:bg-primary-hover text-white"
              >
                {isStreaming
                  ? <Square size={14} fill="currentColor" />
                  : <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"/></svg>
                }
              </button>
            </div>
          </div>
        </div>

        <p className="text-xs text-text-tertiary text-center mt-2">
          Jarvis peut faire des erreurs. Vérifiez les informations importantes.
        </p>
      </div>

      {/* Model dropdown portal */}
      {modelOpen && (
        <ModelDropdown
          anchorEl={modelBtnRef.current}
          onClose={() => setModelOpen(false)}
        />
      )}
    </div>
  )
}
