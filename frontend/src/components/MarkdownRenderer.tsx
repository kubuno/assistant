import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import type { Components } from 'react-markdown'
import 'highlight.js/styles/github-dark.css'

interface Props {
  content: string
}

const components: Components = {
  pre: ({ children }) => (
    <pre className="bg-gray-900 rounded-lg p-4 overflow-x-auto text-sm my-3">{children}</pre>
  ),
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
