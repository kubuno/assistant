import { ChevronDown } from 'lucide-react'
import { useState, useRef, useEffect } from 'react'
import { useJarvisStore } from '../jarvisStore'

export default function ModelSelector() {
  const { models, selectedModel, setSelectedModel } = useJarvisStore()
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  const current = models.find(m => m.id === selectedModel)

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(o => !o)}
        className="flex items-center gap-1.5 text-sm text-text-secondary hover:text-text-primary border border-border rounded-lg px-3 py-1.5 hover:bg-surface-2 transition-colors"
      >
        <span className="max-w-[160px] truncate">{current?.name ?? 'Modèle'}</span>
        <ChevronDown size={14} />
      </button>

      {open && (
        <div className="absolute top-full mt-1 left-0 bg-white border border-border rounded-xl shadow-lg z-50 min-w-[200px] overflow-hidden">
          {models.map(m => (
            <button
              key={m.id}
              onClick={() => { setSelectedModel(m.id); setOpen(false) }}
              className={`w-full text-left px-4 py-2.5 text-sm hover:bg-surface-1 transition-colors ${m.id === selectedModel ? 'text-primary font-medium' : 'text-text-primary'}`}
            >
              {m.name}
              {m.is_default && <span className="ml-2 text-xs text-text-tertiary">(défaut)</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
