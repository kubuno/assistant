import { Plus, Pin, Trash2, Sparkles, Settings2 } from 'lucide-react'
import { useJarvisStore } from '../jarvisStore'
import { ConversationSummary } from '../api'
import { isToday, isYesterday, subDays, isAfter } from 'date-fns'

function groupConversations(convs: ConversationSummary[]) {
  const pinned:   ConversationSummary[] = []
  const today:    ConversationSummary[] = []
  const yesterday:ConversationSummary[] = []
  const week:     ConversationSummary[] = []
  const older:    ConversationSummary[] = []

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

interface ConvItemProps {
  item:    ConversationSummary
  active:  boolean
  onSelect: () => void
  onPin:   () => void
  onDelete:() => void
}

function ConvItem({ item, active, onSelect, onPin, onDelete }: ConvItemProps) {
  const { conversation: c, last_message } = item
  return (
    <div
      onClick={onSelect}
      className={`group relative flex flex-col px-3 py-2 rounded-lg cursor-pointer transition-colors ${
        active ? 'bg-primary/10 text-primary' : 'hover:bg-surface-1 text-text-primary'
      }`}
    >
      <div className="flex items-center justify-between gap-1">
        <span className="text-sm font-medium truncate">{c.title ?? 'Nouvelle conversation'}</span>
        <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0">
          <button
            onClick={e => { e.stopPropagation(); onPin() }}
            className={`p-1 rounded hover:bg-surface-2 ${c.is_pinned ? 'text-primary' : 'text-text-tertiary'}`}
            title={c.is_pinned ? 'Désépingler' : 'Épingler'}
          >
            <Pin size={12} />
          </button>
          <button
            onClick={e => { e.stopPropagation(); onDelete() }}
            className="p-1 rounded hover:bg-danger/10 text-text-tertiary hover:text-danger"
            title="Supprimer"
          >
            <Trash2 size={12} />
          </button>
        </div>
      </div>
      {last_message && (
        <p className="text-xs text-text-tertiary truncate mt-0.5">{last_message}</p>
      )}
    </div>
  )
}

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
    <div className="mb-4">
      <div className="text-xs font-semibold text-text-tertiary uppercase tracking-wide px-3 mb-1">{label}</div>
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

interface Props {
  onSelectConv:    (id: string) => void
  onNewConv:       () => void
  onOpenSettings?: () => void
}

export default function ConversationSidebar({ onSelectConv, onNewConv, onOpenSettings }: Props) {
  const { conversations, activeConvId, togglePin, deleteConversation } = useJarvisStore()
  const groups = groupConversations(conversations)

  return (
    <aside className="w-72 border-r border-border flex flex-col h-full bg-white">
      {/* Header */}
      <div className="flex items-center gap-2 p-4 border-b border-border">
        <div className="w-7 h-7 rounded-full bg-gradient-to-br from-violet-500 to-indigo-600 flex items-center justify-center flex-shrink-0">
          <Sparkles size={14} className="text-white" />
        </div>
        <span className="font-semibold text-text-primary flex-1">Jarvis</span>
        {onOpenSettings && (
          <button
            onClick={onOpenSettings}
            className="p-1 rounded hover:bg-surface-2 text-text-tertiary hover:text-text-primary"
            title="Paramètres Jarvis"
          >
            <Settings2 size={14} />
          </button>
        )}
      </div>

      {/* New conversation */}
      <div className="p-3">
        <button
          onClick={onNewConv}
          className="w-full flex items-center gap-2 px-3 py-2 text-sm font-medium text-primary hover:bg-primary/5 border border-primary/30 rounded-xl transition-colors"
        >
          <Plus size={16} />
          Nouvelle conversation
        </button>
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto px-2 pb-4">
        <Section label="Épinglés"         items={groups.pinned}    activeId={activeConvId} onSelect={onSelectConv} onPin={id => togglePin(id)} onDelete={id => deleteConversation(id)} />
        <Section label="Aujourd'hui"      items={groups.today}     activeId={activeConvId} onSelect={onSelectConv} onPin={id => togglePin(id)} onDelete={id => deleteConversation(id)} />
        <Section label="Hier"             items={groups.yesterday}  activeId={activeConvId} onSelect={onSelectConv} onPin={id => togglePin(id)} onDelete={id => deleteConversation(id)} />
        <Section label="Cette semaine"    items={groups.week}       activeId={activeConvId} onSelect={onSelectConv} onPin={id => togglePin(id)} onDelete={id => deleteConversation(id)} />
        <Section label="Plus anciens"     items={groups.older}      activeId={activeConvId} onSelect={onSelectConv} onPin={id => togglePin(id)} onDelete={id => deleteConversation(id)} />
      </div>
    </aside>
  )
}
