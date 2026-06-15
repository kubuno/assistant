import { useState } from 'react'
import { Plus, Search, Library, Pin, Trash2 } from 'lucide-react'
import { useJarvisStore } from '../jarvisStore'
import { isToday, isYesterday, subDays, isAfter } from 'date-fns'
import type { ConversationSummary } from '../api'

// ── Grouping ──────────────────────────────────────────────────────────────────

function groupConversations(convs: ConversationSummary[]) {
  const pinned:    ConversationSummary[] = []
  const today:     ConversationSummary[] = []
  const yesterday: ConversationSummary[] = []
  const week:      ConversationSummary[] = []
  const older:     ConversationSummary[] = []

  for (const c of convs) {
    if (c.conversation.is_pinned) { pinned.push(c); continue }
    const d = new Date(c.conversation.updated_at)
    if (isToday(d))            today.push(c)
    else if (isYesterday(d))   yesterday.push(c)
    else if (isAfter(d, subDays(new Date(), 7))) week.push(c)
    else                       older.push(c)
  }

  return { pinned, today, yesterday, week, older }
}

// ── Conversation item ─────────────────────────────────────────────────────────

function ConvItem({ item, active, onSelect, onPin, onDelete }: {
  item:     ConversationSummary
  active:   boolean
  onSelect: () => void
  onPin:    () => void
  onDelete: () => void
}) {
  const { conversation: c, last_message } = item
  return (
    <button
      onClick={onSelect}
      className={`group w-full text-left flex flex-col px-3 py-2 rounded-lg cursor-pointer transition-colors
                  ${active ? 'bg-primary/10 text-primary' : 'hover:bg-surface-2 text-text-primary'}`}
    >
      <div className="flex items-center justify-between gap-1 w-full">
        <span className="text-sm truncate flex-1">{c.title ?? 'Nouvelle conversation'}</span>
        <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0">
          <span
            role="button"
            onClick={e => { e.stopPropagation(); onPin() }}
            className={`p-1 rounded hover:bg-surface-3 ${c.is_pinned ? 'text-primary' : 'text-text-tertiary'}`}
            title={c.is_pinned ? 'Désépingler' : 'Épingler'}
          >
            <Pin size={11} />
          </span>
          <span
            role="button"
            onClick={e => { e.stopPropagation(); onDelete() }}
            className="p-1 rounded hover:bg-danger/10 text-text-tertiary hover:text-danger"
            title="Supprimer"
          >
            <Trash2 size={11} />
          </span>
        </div>
      </div>
      {last_message && (
        <p className="text-xs text-text-tertiary truncate mt-0.5">{last_message}</p>
      )}
    </button>
  )
}

// ── Section ───────────────────────────────────────────────────────────────────

function Section({ label, items, activeId, onSelect, onPin, onDelete }: {
  label:    string
  items:    ConversationSummary[]
  activeId: string | null
  onSelect: (id: string) => void
  onPin:    (id: string) => void
  onDelete: (id: string) => void
}) {
  if (!items.length) return null
  return (
    <div className="mb-3">
      <div className="text-xs font-semibold text-text-tertiary uppercase tracking-wide px-3 mb-1">
        {label}
      </div>
      {items.map(item => (
        <ConvItem
          key={item.conversation.id}
          item={item}
          active={item.conversation.id === activeId}
          onSelect={() => onSelect(item.conversation.id)}
          onPin={() => onPin(item.conversation.id)}
          onDelete={() => onDelete(item.conversation.id)}
        />
      ))}
    </div>
  )
}

// ── Main ──────────────────────────────────────────────────────────────────────

export default function JarvisSidebarBody({ collapsed = false }: { collapsed?: boolean }) {
  const {
    conversations, activeConvId,
    setActiveConv, createConversation,
    togglePin, deleteConversation,
  } = useJarvisStore()

  const [search, setSearch] = useState('')

  const filtered = search.trim()
    ? conversations.filter(c =>
        c.conversation.title?.toLowerCase().includes(search.toLowerCase()) ||
        c.last_message?.toLowerCase().includes(search.toLowerCase())
      )
    : conversations

  const groups = groupConversations(filtered)

  async function handleNewConv() {
    const id = await createConversation()
    setActiveConv(id)
  }

  // Replié : icône « nouvelle conversation » (la liste de discussions ne tient pas).
  if (collapsed) {
    return (
      <nav className="flex flex-col items-center px-2 py-2 gap-1">
        <button
          onClick={handleNewConv}
          title="Nouvelle conversation"
          className="w-10 h-10 flex items-center justify-center bg-white rounded-full transition-shadow"
          style={{ boxShadow: '0 1px 3px rgba(60,64,67,0.3), 0 4px 8px rgba(60,64,67,0.15)' }}
        >
          <Plus size={20} className="text-text-secondary" />
        </button>
      </nav>
    )
  }

  return (
    <>
      {/* Nouvelle conversation */}
      <div className="px-3 mb-3">
        <button
          onClick={handleNewConv}
          className="flex items-center gap-2 bg-white text-sm font-medium text-text-primary
                     cursor-pointer w-full hover:shadow-md transition-shadow"
          style={{
            padding:      '20px 25px',
            border:       '1px solid #e0e0e0',
            borderRadius: '20px',
            boxShadow:    '0 1px 3px rgba(0,0,0,0.12)',
          }}
        >
          <Plus size={20} className="text-text-secondary" />
          Nouvelle conversation
        </button>
      </div>

      {/* Quick nav */}
      <nav className="px-3 mb-1 space-y-0.5">
        <button
          className="w-full flex items-center gap-3 px-3 py-2 rounded-full text-sm text-text-secondary hover:bg-surface-2 transition-colors"
          onClick={() => {
            document.getElementById('jarvis-search-input')?.focus()
          }}
        >
          <Search size={18} />
          Rechercher
        </button>
        <button className="w-full flex items-center gap-3 px-3 py-2 rounded-full text-sm text-text-secondary hover:bg-surface-2 transition-colors">
          <Library size={18} />
          Bibliothèque
        </button>
      </nav>

      {/* Search input (hidden until focused via above button or typing) */}
      <div className="px-3 mb-2">
        <input
          id="jarvis-search-input"
          type="text"
          value={search}
          onChange={e => setSearch(e.target.value)}
          placeholder="Rechercher dans les discussions…"
          className="w-full text-sm border border-border rounded-full px-3 py-1.5
                     focus:outline-none focus:border-primary bg-surface-1 text-text-primary
                     placeholder:text-text-tertiary"
          style={{ display: search ? 'block' : 'none' }}
        />
      </div>

      {/* Conversation list */}
      <div className="flex-1 overflow-y-auto px-1 pb-4">
        {search ? (
          <div className="mb-2 px-3 text-xs font-semibold text-text-tertiary uppercase tracking-wide">
            Résultats
          </div>
        ) : null}

        {search ? (
          filtered.map(item => (
            <ConvItem
              key={item.conversation.id}
              item={item}
              active={item.conversation.id === activeConvId}
              onSelect={() => { setActiveConv(item.conversation.id); setSearch('') }}
              onPin={() => togglePin(item.conversation.id)}
              onDelete={() => deleteConversation(item.conversation.id)}
            />
          ))
        ) : (
          <>
            <Section label="Épinglés"       items={groups.pinned}    activeId={activeConvId} onSelect={id => setActiveConv(id)} onPin={id => togglePin(id)} onDelete={id => deleteConversation(id)} />
            <Section label="Aujourd'hui"    items={groups.today}     activeId={activeConvId} onSelect={id => setActiveConv(id)} onPin={id => togglePin(id)} onDelete={id => deleteConversation(id)} />
            <Section label="Hier"           items={groups.yesterday}  activeId={activeConvId} onSelect={id => setActiveConv(id)} onPin={id => togglePin(id)} onDelete={id => deleteConversation(id)} />
            <Section label="Cette semaine"  items={groups.week}       activeId={activeConvId} onSelect={id => setActiveConv(id)} onPin={id => togglePin(id)} onDelete={id => deleteConversation(id)} />
            <Section label="Plus anciens"   items={groups.older}      activeId={activeConvId} onSelect={id => setActiveConv(id)} onPin={id => togglePin(id)} onDelete={id => deleteConversation(id)} />
          </>
        )}

        {!conversations.length && (
          <p className="text-xs text-text-tertiary text-center mt-6 px-3">
            Aucune conversation pour l'instant
          </p>
        )}
      </div>
    </>
  )
}
