import type { AgentSuggestion } from '../api'

interface Props {
  onSelect: (prompt: string) => void
  suggestions?: AgentSuggestion[]
}

const DEFAULT: AgentSuggestion[] = [
  { label: 'Explique un concept', prompt: 'Explique-moi simplement le concept de récursivité en programmation.', icon: '💡' },
  { label: 'Aide au code',        prompt: 'Aide-moi à déboguer ce code et explique le problème :', icon: '💻' },
  { label: 'Résume un texte',     prompt: 'Résume le texte suivant en 3 points clés :', icon: '📝' },
  { label: 'Plan de projet',      prompt: 'Aide-moi à créer un plan détaillé pour le projet suivant :', icon: '📋' },
]

export default function PromptSuggestions({ onSelect, suggestions }: Props) {
  const items = suggestions && suggestions.length ? suggestions : DEFAULT
  return (
    <div className="grid grid-cols-2 gap-3 max-w-2xl mx-auto">
      {items.map(s => (
        <button
          key={s.label}
          onClick={() => onSelect(s.prompt)}
          className="text-left p-4 rounded-xl border border-border hover:border-primary hover:bg-primary/5 transition-all group"
        >
          <div className="flex items-center gap-2 text-sm font-medium text-text-primary group-hover:text-primary transition-colors">
            {s.icon && <span className="text-base">{s.icon}</span>}
            {s.label}
          </div>
          <div className="text-xs text-text-tertiary mt-1 leading-relaxed truncate">
            {s.prompt}
          </div>
        </button>
      ))}
    </div>
  )
}
