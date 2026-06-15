import { useEffect } from 'react'
import { useJarvisStore } from '../jarvisStore'
import MessageList from './MessageList'
import ChatInput from './ChatInput'

interface Props {
  convId: string
}

export default function ConversationPage({ convId }: Props) {
  const { fetchMessages } = useJarvisStore()

  useEffect(() => {
    fetchMessages(convId)
  }, [convId, fetchMessages])

  return (
    <div className="flex flex-col h-full">
      <MessageList />
      <ChatInput convId={convId} />
    </div>
  )
}
