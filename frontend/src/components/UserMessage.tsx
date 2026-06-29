import { useEffect, useRef, useState } from 'react'
import { Copy, Check, Pencil } from 'lucide-react'
import { useAuthStore } from '@kubuno/sdk'
import { useJarvisStore } from '../jarvisStore'

interface Props {
  id:      string
  content: string
}

export default function UserMessage({ id, content }: Props) {
  const user = useAuthStore(s => s.user)
  const { activeConvId, editUserMessage, isStreaming } = useJarvisStore()
  const [copied, setCopied] = useState(false)
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(content)
  const taRef = useRef<HTMLTextAreaElement>(null)

  // Auto-resize the edit textarea to fit its content while typing.
  useEffect(() => {
    if (!editing) return
    const ta = taRef.current
    if (!ta) return
    ta.style.height = 'auto'
    ta.style.height = `${Math.min(ta.scrollHeight, 400)}px`
  }, [draft, editing])

  const initials = (user?.display_name ?? user?.username ?? 'U')
    .split(' ').map(w => w[0]).join('').slice(0, 2).toUpperCase()

  const copy = async () => {
    try { await navigator.clipboard.writeText(content); setCopied(true); setTimeout(() => setCopied(false), 2000) }
    catch { /* ignore */ }
  }

  const startEdit = () => { setDraft(content); setEditing(true) }
  const cancel = () => { setEditing(false); setDraft(content) }
  const save = () => {
    const c = draft.trim()
    setEditing(false)
    if (c && c !== content && activeConvId) editUserMessage(activeConvId, id, c)
  }

  // ── Mode édition : éditeur pleine largeur, hauteur auto ──
  if (editing) {
    return (
      <div className="w-full">
        <div className="bg-white border border-border rounded-2xl px-4 py-3 shadow-sm focus-within:border-primary/50 transition-colors">
          <textarea
            ref={taRef}
            autoFocus
            value={draft}
            onChange={e => setDraft(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); save() }
              else if (e.key === 'Escape') { e.preventDefault(); cancel() }
            }}
            rows={1}
            className="w-full resize-none bg-transparent text-sm text-text-primary leading-relaxed focus:outline-none max-h-[400px]"
          />
          <div className="flex items-center justify-end gap-2 mt-2">
            <button onClick={cancel}
              className="text-sm font-medium text-text-secondary hover:text-text-primary px-4 py-1.5 rounded-full hover:bg-surface-2 transition-colors">
              Annuler
            </button>
            <button onClick={save} disabled={!draft.trim() || isStreaming}
              className="text-sm font-medium text-white bg-primary hover:bg-primary-hover disabled:opacity-40 px-4 py-1.5 rounded-full transition-colors">
              Envoyer
            </button>
          </div>
        </div>
      </div>
    )
  }

  // ── Affichage normal ──
  return (
    <div className="group flex flex-col items-end">
      <div className="flex gap-3 justify-end w-full">
        <div className="max-w-[75%] min-w-0">
          <div className="bg-primary text-white rounded-2xl rounded-tr-md px-4 py-3 text-sm leading-relaxed whitespace-pre-wrap break-words">
            {content}
          </div>
        </div>
        <div className="flex-shrink-0 w-8 h-8 rounded-full bg-primary text-white text-xs font-medium flex items-center justify-center">
          {initials}
        </div>
      </div>

      {/* Actions au survol (sous la bulle, alignées sous le texte) */}
      <div className="flex items-center gap-0.5 mt-1 pr-11 text-text-secondary opacity-0 group-hover:opacity-100 transition-opacity">
        <button onClick={copy} title="Copier le message" aria-label="Copier le message" className="p-1.5 rounded-lg transition-colors hover:bg-surface-2 hover:text-text-primary">
          {copied ? <Check size={14} className="text-success" /> : <Copy size={14} />}
        </button>
        <button onClick={startEdit} title="Modifier le message" aria-label="Modifier le message" className="p-1.5 rounded-lg transition-colors hover:bg-surface-2 hover:text-text-primary">
          <Pencil size={14} />
        </button>
      </div>
    </div>
  )
}
