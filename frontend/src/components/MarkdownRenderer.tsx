import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import type { Components } from 'react-markdown'
import { Copy, Check } from 'lucide-react'
import { useState, type ReactNode, isValidElement } from 'react'
import 'highlight.js/styles/github-dark.css'

interface Props {
  content: string
}

// Extrait le texte brut d'un arbre de nœuds React (les blocs surlignés par
// rehype-highlight contiennent des <span> imbriqués) — pour le bouton « Copier ».
function nodeText(node: ReactNode): string {
  if (node == null || node === false) return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(nodeText).join('')
  if (isValidElement(node)) return nodeText((node.props as { children?: ReactNode }).children)
  return ''
}

// Bloc de code : étiquette de langage + bouton « Copier » au survol.
function CodeBlock({ children }: { children: ReactNode }) {
  const [copied, setCopied] = useState(false)
  const codeEl = isValidElement(children) ? (children.props as { className?: string; children?: ReactNode }) : null
  const lang = codeEl?.className?.match(/language-([\w+-]+)/)?.[1]
  const raw = nodeText(children).replace(/\n$/, '')
  const copy = async () => {
    try { await navigator.clipboard.writeText(raw); setCopied(true); setTimeout(() => setCopied(false), 1800) } catch { /* ignore */ }
  }
  return (
    <div className="my-3 rounded-lg overflow-hidden">
      <div className="flex items-center justify-between px-3 h-7 bg-gray-800 text-[11px] text-gray-400 font-mono">
        <span>{lang ?? 'code'}</span>
        <button onClick={copy} className="flex items-center gap-1 hover:text-gray-100 transition-colors">
          {copied ? <Check size={11} className="text-emerald-400" /> : <Copy size={11} />}
          {copied ? 'Copié' : 'Copier'}
        </button>
      </div>
      <pre className="bg-gray-900 p-4 overflow-x-auto text-sm">{children}</pre>
    </div>
  )
}

const components: Components = {
  pre: ({ children }) => <CodeBlock>{children}</CodeBlock>,
  code: ({ children, className, ...props }) => {
    const isBlock = className?.startsWith('language-')
    return isBlock
      ? <code className={className} {...props}>{children}</code>
      : <code className="bg-gray-100 text-rose-600 px-1 py-0.5 rounded text-sm font-mono" {...props}>{children}</code>
  },
}

export default function MarkdownRenderer({ content }: Props) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeHighlight]}
      components={{
        ...components,
        p: ({ children }) => <p className="mb-3 last:mb-0 leading-relaxed">{children}</p>,
        ul: ({ children }) => <ul className="list-disc pl-5 mb-3 space-y-1">{children}</ul>,
        ol: ({ children }) => <ol className="list-decimal pl-5 mb-3 space-y-1">{children}</ol>,
        li: ({ children }) => <li className="leading-relaxed">{children}</li>,
        h1: ({ children }) => <h1 className="text-xl font-bold mb-3 mt-4">{children}</h1>,
        h2: ({ children }) => <h2 className="text-lg font-bold mb-2 mt-4">{children}</h2>,
        h3: ({ children }) => <h3 className="font-semibold mb-2 mt-3">{children}</h3>,
        a: ({ children, href }) => <a href={href} target="_blank" rel="noopener noreferrer" className="text-primary underline hover:opacity-80">{children}</a>,
        blockquote: ({ children }) => (
          <blockquote className="border-l-4 border-primary pl-4 italic text-text-secondary my-3">
            {children}
          </blockquote>
        ),
        table: ({ children }) => (
          <div className="overflow-x-auto my-3">
            <table className="min-w-full border border-border text-sm">{children}</table>
          </div>
        ),
        th: ({ children }) => (
          <th className="bg-surface-2 border border-border px-3 py-2 text-left font-medium">{children}</th>
        ),
        td: ({ children }) => (
          <td className="border border-border px-3 py-2">{children}</td>
        ),
      }}
    >
      {content}
    </ReactMarkdown>
  )
}
