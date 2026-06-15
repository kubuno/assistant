import { useEffect } from 'react'
import { useJarvisStore } from './jarvisStore'
import ConversationPage from './components/ConversationPage'
import HomePage from './components/HomePage'

export default function JarvisPage() {
  const { activeConvId, setActiveConv, fetchConversations, fetchAgents, fetchModels } = useJarvisStore()

  useEffect(() => {
    fetchConversations()
    fetchAgents()
    fetchModels()
  }, [fetchConversations, fetchAgents, fetchModels])

  return (
    <div className="flex h-full overflow-hidden">
      <main className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {activeConvId ? (
          <ConversationPage key={activeConvId} convId={activeConvId} />
        ) : (
          <HomePage onConvCreated={id => setActiveConv(id)} />
        )}
      </main>
    </div>
  )
}
