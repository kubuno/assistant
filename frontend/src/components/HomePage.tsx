import { Sparkles } from 'lucide-react'
import { useEffect } from 'react'
import { useAuthStore } from '@kubuno/sdk'
import PromptSuggestions from './PromptSuggestions'
import ChatInput from './ChatInput'
import { useJarvisStore } from '../jarvisStore'

interface Props {
  onConvCreated: (convId: string) => void
}

export default function HomePage({ onConvCreated }: Props) {
  const user = useAuthStore(s => s.user)
  const firstName = user?.display_name?.split(' ')[0] ?? user?.username ?? 'là'

  const { agents, selectedAgentId, setSelectedAgentId } = useJarvisStore()
  const activeAgent = agents.find(a => a.id === selectedAgentId) ?? agents[0]

  // Sélectionne le 1ᵉʳ agent par défaut (persona des nouvelles conversations).
  useEffect(() => {
    if (!selectedAgentId && agents.length) setSelectedAgentId(agents[0].id)
  }, [agents, selectedAgentId, setSelectedAgentId])

  const fillInput = (prompt: string) => {
    const textarea = document.getElementById('jarvis-home-input') as HTMLTextAreaElement | null
    if (!textarea) return
    const setter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value')?.set
    setter?.call(textarea, prompt)
    textarea.dispatchEvent(new Event('input', { bubbles: true }))
    textarea.focus()
  }

  return (
    <div className="flex flex-col h-full relative overflow-hidden">
      {/* Gemini-style gradient background */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          background: 'radial-gradient(ellipse 80% 60% at 50% -10%, rgba(103,58,183,0.08) 0%, transparent 70%)',
        }}
      />

      {/* Center content */}
      <div className="flex-1 flex flex-col items-center justify-center px-6 pb-4 relative z-10">
        {/* Avatar de l'agent actif */}
        <div
          className="w-16 h-16 rounded-full flex items-center justify-center mb-6 text-3xl"
          style={{ background: activeAgent?.avatar_color ?? 'linear-gradient(135deg, #7c3aed, #4f46e5)' }}
        >
          {activeAgent?.avatar_emoji ?? <Sparkles size={30} className="text-white" />}
        </div>

        {/* Greeting */}
        <h1 className="text-3xl font-normal text-text-primary mb-1 text-center" style={{ fontWeight: 400 }}>
          Bonjour, {firstName}
        </h1>
        <p className="text-lg text-text-secondary mb-6 text-center" style={{ fontWeight: 300 }}>
          {activeAgent?.description || 'Par où commencer ?'}
        </p>

        {/* Sélecteur d'agent (persona) */}
        {agents.length > 1 && (
          <div className="flex flex-wrap items-center justify-center gap-2 mb-8">
            {agents.map(a => {
              const active = a.id === activeAgent?.id
              return (
                <button
                  key={a.id}
                  onClick={() => setSelectedAgentId(a.id)}
                  className={`flex items-center gap-1.5 h-8 pl-2 pr-3 rounded-full border text-sm transition-colors ${
                    active ? 'border-primary bg-primary/10 text-primary font-medium' : 'border-border text-text-secondary hover:bg-surface-2'
                  }`}
                >
                  <span className="w-5 h-5 rounded-full flex items-center justify-center text-xs"
                    style={{ background: a.avatar_color ?? '#e0e0e0' }}>{a.avatar_emoji ?? '🤖'}</span>
                  {a.name}
                </button>
              )
            })}
          </div>
        )}

        {/* Suggestions de prompts (propres à l'agent) */}
        <PromptSuggestions onSelect={fillInput} suggestions={activeAgent?.prompt_suggestions} />
      </div>

      {/* Input at bottom */}
      <div className="relative z-10 flex-shrink-0">
        <ChatInput onConvCreated={onConvCreated} inputId="jarvis-home-input" />
      </div>
    </div>
  )
}
