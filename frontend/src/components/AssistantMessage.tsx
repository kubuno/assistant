import { Sparkles, Copy, Check, ThumbsUp, ThumbsDown, RotateCcw, Trash2, Share2, MoreHorizontal, Volume2, Split } from 'lucide-react'
import { useState } from 'react'
import { MenuDropdown, useMenuDropdown, type MenuItem } from '@ui'
import MarkdownRenderer from './MarkdownRenderer'
import ToolCallCard from './ToolCallCard'
import { useJarvisStore } from '../jarvisStore'
import type { JarvisToolCall } from '../api'

interface Props {
  content:   string
  streaming?: boolean
  toolCalls?: JarvisToolCall[]
  messageId?: string
  createdAt?: string
  promptTokens?: number
  completionTokens?: number
  feedback?: 'like' | 'dislike' | null
  isLast?: boolean
}

function fmtDateTime(iso?: string): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (isNaN(d.getTime())) return ''
  return `${d.toLocaleDateString('fr-FR', { day: 'numeric', month: 'long' })}, ${d.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })}`
}

export default function AssistantMessage({ content, streaming, toolCalls, messageId, createdAt, feedback, isLast }: Props) {
  const [copied, setCopied] = useState(false)
  const { activeConvId, setMessageFeedback, regenerate, deleteMessage, isStreaming } = useJarvisStore()
  const more = useMenuDropdown()

  const copyToClipboard = async () => {
    try { await navigator.clipboard.writeText(content); setCopied(true); setTimeout(() => setCopied(false), 2000) } catch { /* ignore */ }
  }
  // Lecture à haute voix via la synthèse vocale du navigateur (Web Speech, FR).
  const speak = () => {
    try {
      window.speechSynthesis.cancel()
      const u = new SpeechSynthesisUtterance(content)
      u.lang = 'fr-FR'
      window.speechSynthesis.speak(u)
    } catch { /* indisponible */ }
  }
  const iconBtn = 'flex items-center justify-center w-7 h-7 rounded-lg transition-colors'

  const moreItems: MenuItem[] = [
    ...(createdAt ? [{ type: 'label' as const, text: fmtDateTime(createdAt) }] : []),
    { type: 'action', label: 'Passer dans un nouveau chat', icon: <Split size={16} />, disabled: true, onClick: () => {} },
    { type: 'action', label: 'Lire à haute voix', icon: <Volume2 size={16} />, onClick: speak },
    ...(messageId && activeConvId
      ? [{ type: 'separator' as const },
         { type: 'action' as const, label: 'Supprimer', icon: <Trash2 size={16} />, danger: true, onClick: () => deleteMessage(activeConvId, messageId) }]
      : []),
  ]

  return (
    <div className="flex gap-3">
      <div className="flex-shrink-0 w-8 h-8 rounded-full bg-gradient-to-br from-violet-500 to-indigo-600 flex items-center justify-center">
        <Sparkles size={14} className="text-white" />
      </div>
      <div className="flex-1 min-w-0 group">
        {toolCalls && toolCalls.length > 0 && (
          <div className="mb-2 space-y-1.5">
            {toolCalls.map((call, i) => <ToolCallCard key={i} call={call} />)}
          </div>
        )}
        <div className="text-sm text-text-primary">
          <MarkdownRenderer content={content} />
          {streaming && (
            <span className="inline-block w-0.5 h-4 bg-primary animate-pulse ml-0.5 align-middle" />
          )}
        </div>
        {!streaming && content && (
          <div className="mt-2 flex items-center gap-0.5 text-text-secondary opacity-0 group-hover:opacity-100 transition-opacity">
            <button onClick={copyToClipboard} title="Copier la réponse" aria-label="Copier la réponse"
              className={`${iconBtn} hover:text-text-primary hover:bg-surface-2`}>
              {copied ? <Check size={15} className="text-success" /> : <Copy size={15} />}
            </button>
            {messageId && activeConvId && (
              <>
                <button onClick={() => setMessageFeedback(activeConvId, messageId, 'like')} title="Bonne réponse" aria-label="Bonne réponse"
                  className={`${iconBtn} hover:bg-surface-2 ${feedback === 'like' ? 'text-primary' : 'hover:text-text-primary'}`}>
                  <ThumbsUp size={15} />
                </button>
                <button onClick={() => setMessageFeedback(activeConvId, messageId, 'dislike')} title="Mauvaise réponse" aria-label="Mauvaise réponse"
                  className={`${iconBtn} hover:bg-surface-2 ${feedback === 'dislike' ? 'text-danger' : 'hover:text-text-primary'}`}>
                  <ThumbsDown size={15} />
                </button>
              </>
            )}
            <button title="Partager" aria-label="Partager" disabled
              className={`${iconBtn} opacity-40 cursor-not-allowed`}>
              <Share2 size={15} />
            </button>
            {isLast && activeConvId && (
              <button onClick={() => regenerate(activeConvId)} disabled={isStreaming} title="Réessayer" aria-label="Réessayer"
                className={`${iconBtn} hover:text-text-primary hover:bg-surface-2 disabled:opacity-40`}>
                <RotateCcw size={15} />
              </button>
            )}
            <button onClick={more.open} title="Plus d'actions" aria-label="Plus d'actions"
              className={`${iconBtn} hover:text-text-primary hover:bg-surface-2`}>
              <MoreHorizontal size={15} />
            </button>
            {more.isOpen && more.pos && (
              <MenuDropdown items={moreItems} pos={more.pos} onClose={more.close} minWidth={240} />
            )}
          </div>
        )}
      </div>
    </div>
  )
}
