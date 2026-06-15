import { useEffect, useRef } from 'react'
import { useJarvisStore } from '../jarvisStore'
import UserMessage from './UserMessage'
import AssistantMessage from './AssistantMessage'
import ThinkingIndicator from './ThinkingIndicator'

export default function MessageList() {
  const { messages, isStreaming, streamingText } = useJarvisStore()
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, streamingText])

  return (
    <div className="flex-1 overflow-y-auto px-4 py-6">
      <div className="max-w-3xl mx-auto space-y-6">
        {messages.map(msg => (
          msg.role === 'user'
            ? <UserMessage key={msg.id} content={msg.content} />
            : <AssistantMessage key={msg.id} content={msg.content} />
        ))}

        {isStreaming && streamingText && (
          <AssistantMessage content={streamingText} streaming />
        )}

        {isStreaming && !streamingText && (
          <ThinkingIndicator />
        )}

        <div ref={bottomRef} />
      </div>
    </div>
  )
}
