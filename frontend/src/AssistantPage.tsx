import { useEffect } from 'react'
import { useAssistantStore } from './assistantStore'
import ConversationPage from './components/ConversationPage'
import HomePage from './components/HomePage'

export default function AssistantPage() {
  const { activeConvId, setActiveConv, fetchConversations, fetchAgents, fetchModels, fetchFolders } = useAssistantStore()

  useEffect(() => {
    fetchConversations()
    fetchAgents()
    fetchModels()
    fetchFolders()
  }, [fetchConversations, fetchAgents, fetchModels, fetchFolders])

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
