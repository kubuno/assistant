import { useEffect, useRef } from 'react'
import { useJarvisStore } from '../jarvisStore'
import UserMessage from './UserMessage'
import AssistantMessage from './AssistantMessage'
import ThinkingIndicator from './ThinkingIndicator'

export default function MessageList() {
  const { messages, isStreaming, streamingText, streamingToolCalls } = useJarvisStore()
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, streamingText, streamingToolCalls])

  const hasStreamingContent = !!streamingText || streamingToolCalls.length > 0

  return (
    <div className="flex-1 overflow-y-auto px-4 py-6">
      <div className="max-w-3xl mx-auto space-y-6">
        {messages.map((msg, i) => (
          msg.role === 'user'
            ? <UserMessage key={msg.id} id={msg.id} content={msg.content} />
            : <AssistantMessage
                key={msg.id}
                messageId={msg.id}
                content={msg.content}
                toolCalls={msg.tool_calls}
                createdAt={msg.created_at}
                promptTokens={msg.prompt_tokens}
                completionTokens={msg.completion_tokens}
                feedback={msg.feedback ?? null}
                isLast={i === messages.length - 1 && !isStreaming}
              />
        ))}

        {isStreaming && hasStreamingContent && (
          <AssistantMessage content={streamingText} streaming toolCalls={streamingToolCalls} />
        )}

        {isStreaming && !hasStreamingContent && (
          <ThinkingIndicator />
        )}

        <div ref={bottomRef} />
      </div>
    </div>
  )
}
