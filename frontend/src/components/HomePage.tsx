import { Sparkles } from 'lucide-react'
import { useAuthStore } from '@kubuno/sdk'
import PromptSuggestions from './PromptSuggestions'
import ChatInput from './ChatInput'

interface Props {
  onConvCreated: (convId: string) => void
}

export default function HomePage({ onConvCreated }: Props) {
  const user = useAuthStore(s => s.user)
  const firstName = user?.display_name?.split(' ')[0] ?? user?.username ?? 'là'

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
        {/* Jarvis avatar */}
        <div
          className="w-16 h-16 rounded-full flex items-center justify-center mb-8"
          style={{ background: 'linear-gradient(135deg, #7c3aed, #4f46e5)' }}
        >
          <Sparkles size={30} className="text-white" />
        </div>

        {/* Greeting */}
        <h1 className="text-3xl font-normal text-text-primary mb-1 text-center" style={{ fontWeight: 400 }}>
          Bonjour, {firstName}
        </h1>
        <p className="text-xl text-text-secondary mb-10 text-center" style={{ fontWeight: 300 }}>
          Par où commencer ?
        </p>

        {/* Suggestion cards */}
        <PromptSuggestions onSelect={async (prompt) => {
          // Trigger a new conv with the suggestion pre-filled
          // We pass it as the initial message via ChatInput's submit
          const textarea = document.getElementById('jarvis-home-input') as HTMLTextAreaElement | null
          if (textarea) {
            const nativeInputValueSetter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value')?.set
            nativeInputValueSetter?.call(textarea, prompt)
            textarea.dispatchEvent(new Event('input', { bubbles: true }))
            textarea.focus()
          }
        }} />
      </div>

      {/* Input at bottom */}
      <div className="relative z-10 flex-shrink-0">
        <ChatInput onConvCreated={onConvCreated} inputId="jarvis-home-input" />
      </div>
    </div>
  )
}
