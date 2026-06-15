import { Sparkles, Copy, Check } from 'lucide-react'
import { useState } from 'react'
import MarkdownRenderer from './MarkdownRenderer'

interface Props {
  content:   string
  streaming?: boolean
}

export default function AssistantMessage({ content, streaming }: Props) {
  const [copied, setCopied] = useState(false)

  const copyToClipboard = async () => {
    await navigator.clipboard.writeText(content)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="flex gap-3">
      <div className="flex-shrink-0 w-8 h-8 rounded-full bg-gradient-to-br from-violet-500 to-indigo-600 flex items-center justify-center">
        <Sparkles size={14} className="text-white" />
      </div>
      <div className="flex-1 min-w-0 group">
        <div className="text-sm text-text-primary">
          <MarkdownRenderer content={content} />
          {streaming && (
            <span className="inline-block w-0.5 h-4 bg-primary animate-pulse ml-0.5 align-middle" />
          )}
        </div>
        {!streaming && content && (
          <div className="mt-2 opacity-0 group-hover:opacity-100 transition-opacity">
            <button
              onClick={copyToClipboard}
              className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors"
            >
              {copied ? <Check size={12} className="text-success" /> : <Copy size={12} />}
              {copied ? 'Copié' : 'Copier'}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
