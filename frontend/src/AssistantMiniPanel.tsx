import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { MessageSquarePlus, Pin } from 'lucide-react'
import { Button, Spinner } from '@ui'
import { assistantApi } from './api'
import { hashTo } from './hashRoute'

/**
 * Assistant side panel — pick up a conversation from wherever you are.
 *
 * It lists conversations and hands off to the module; it does NOT embed a second
 * chat surface. Streaming answers, model choice, attachments and agent selection
 * all live in the module, and a half-width copy of them would be the worst place
 * to hold a conversation you actually care about.
 */
export default function AssistantMiniPanel() {
  const navigate = useNavigate()

  const { data: conversations = [], isLoading } = useQuery({
    queryKey: ['assistant-mini-conversations'],
    queryFn:  () => assistantApi.listConversations(),
  })

  const recent = conversations
    .filter(c => !c.conversation.is_archived)
    .sort((a, b) => Number(b.conversation.is_pinned) - Number(a.conversation.is_pinned))
    .slice(0, 10)

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex-shrink-0 px-3 pt-3 pb-2">
        <Button icon={<MessageSquarePlus size={15} />} className="w-full" onClick={() => navigate('/assistant')}>
          Nouvelle conversation
        </Button>
      </div>

      <div className="px-4 pb-1 uppercase tracking-wide text-text-tertiary" style={{ fontSize: 'var(--kb-text-meta)' }}>
        Conversations
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
        {isLoading ? (
          <div className="flex justify-center py-6"><Spinner /></div>
        ) : recent.length === 0 ? (
          <p className="px-2 py-4 text-text-tertiary" style={{ fontSize: 'var(--kb-text-meta)' }}>
            Aucune conversation pour l’instant.
          </p>
        ) : (
          <ul className="space-y-0.5">
            {recent.map(({ conversation, last_message }) => (
              <li key={conversation.id}>
                <button
                  type="button"
                  onClick={() => navigate(hashTo('conversation', conversation.id))}
                  className="flex w-full items-start gap-2 rounded-lg px-2 py-1.5 text-left transition-colors
                             hover:bg-surface-1 focus:outline-none focus-visible:ring-2 focus-visible:ring-primary"
                >
                  {conversation.is_pinned && <Pin size={13} className="mt-0.5 flex-shrink-0 text-text-tertiary" />}
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-text-primary" style={{ fontSize: 'var(--kb-text-body)' }}>
                      {conversation.title || 'Sans titre'}
                    </span>
                    {last_message && (
                      <span className="block truncate text-text-tertiary" style={{ fontSize: 'var(--kb-text-meta)' }}>
                        {last_message}
                      </span>
                    )}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}
