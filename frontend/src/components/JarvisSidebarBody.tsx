import { useRef, useState } from 'react'
import { Plus, Search, Library, Pin, Trash2, Pencil, FolderPlus, FolderInput, ChevronRight, Folder, Check, X, Share2, UserPlus, Archive } from 'lucide-react'
import { MenuDropdown, useMenuDropdown, type MenuItem } from '@ui'
import { prompt } from '@kubuno/sdk'
import { useJarvisStore } from '../jarvisStore'
import { isToday, isYesterday, subDays, isAfter } from 'date-fns'
import type { ConversationSummary, JarvisFolder } from '../api'

// Manual order first (drag-and-drop position), recency as tiebreak. Pinned float up.
function convCmp(a: ConversationSummary, b: ConversationSummary) {
  const A = a.conversation, B = b.conversation
  if (A.is_pinned !== B.is_pinned) return A.is_pinned ? -1 : 1
  if (A.position !== B.position) return A.position - B.position
  return new Date(B.updated_at).getTime() - new Date(A.updated_at).getTime()
}

// ── Grouping (conversations sans dossier, par date) ─────────────────────────────

function groupConversations(convs: ConversationSummary[]) {
  const pinned: ConversationSummary[] = []
  const today: ConversationSummary[] = []
  const yesterday: ConversationSummary[] = []
  const week: ConversationSummary[] = []
  const older: ConversationSummary[] = []
  for (const c of convs) {
    if (c.conversation.is_pinned) { pinned.push(c); continue }
    const d = new Date(c.conversation.updated_at)
    if (isToday(d)) today.push(c)
    else if (isYesterday(d)) yesterday.push(c)
    else if (isAfter(d, subDays(new Date(), 7))) week.push(c)
    else older.push(c)
  }
  return { pinned, today, yesterday, week, older }
}

// ── Conversation item (renommer / déplacer / épingler / supprimer) ──────────────

function ConvItem({ item, active, folders, onSelect, onPin, onDelete, onRename, onMove, onArchive, onNewProject }: {
  item: ConversationSummary
  active: boolean
  folders: JarvisFolder[]
  onSelect: () => void
  onPin: () => void
  onDelete: () => void
  onRename: (title: string) => void
  onMove: (folderId: string | null) => void
  onArchive: () => void
  onNewProject: () => void
}) {
  const { conversation: c } = item
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(c.title ?? '')
  const ctx = useMenuDropdown()

  const commit = () => { setEditing(false); if (draft.trim() && draft !== c.title) onRename(draft.trim()) }

  // Sous-menu « Déplacer vers le projet » : nouveau projet, sortie de projet, projets.
  const moveSub: MenuItem[] = [
    { type: 'action', label: 'Nouveau projet', icon: <FolderPlus size={16} />, onClick: onNewProject },
    ...(c.folder_id ? [{ type: 'action' as const, label: 'Aucun projet', onClick: () => onMove(null) }] : []),
    ...(folders.length ? [{ type: 'separator' as const }] : []),
    ...folders.map(f => ({
      type: 'action' as const, label: f.name,
      icon: <Folder size={16} style={{ color: f.color ?? undefined }} />,
      checked: c.folder_id === f.id,
      onClick: () => onMove(f.id),
    })),
  ]

  const ctxItems: MenuItem[] = [
    { type: 'action', label: 'Partager', icon: <Share2 size={16} />, disabled: true, onClick: () => {} },
    { type: 'action', label: 'Démarrer une conversation de groupe', icon: <UserPlus size={16} />, disabled: true, onClick: () => {} },
    { type: 'action', label: 'Renommer', icon: <Pencil size={16} />, onClick: () => { setDraft(c.title ?? ''); setEditing(true) } },
    { type: 'submenu', label: 'Déplacer vers le projet', icon: <FolderInput size={16} />, items: moveSub },
    { type: 'separator' },
    { type: 'action', label: c.is_pinned ? 'Désépingler le chat' : 'Épingler le chat', icon: <Pin size={16} />, onClick: onPin },
    { type: 'action', label: 'Archiver', icon: <Archive size={16} />, onClick: onArchive },
    { type: 'action', label: 'Supprimer', icon: <Trash2 size={16} />, danger: true, onClick: onDelete },
  ]

  return (
    <div
      onContextMenu={e => { e.preventDefault(); ctx.open(e) }}
      className={`group relative flex items-center gap-1.5 px-3 py-2 rounded-lg transition-colors cursor-grab active:cursor-grabbing
                  ${active ? 'bg-primary/10 text-primary' : 'text-text-primary hover:bg-surface-3'}`}
    >
      {editing ? (
        <input
          autoFocus value={draft}
          onChange={e => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); commit() } else if (e.key === 'Escape') { setEditing(false); setDraft(c.title ?? '') } }}
          onClick={e => e.stopPropagation()}
          className="flex-1 text-sm bg-white border border-primary rounded px-1.5 py-0.5 focus:outline-none text-text-primary"
        />
      ) : (
        <button onClick={onSelect} className="text-sm truncate flex-1 text-left">{c.title ?? 'Nouvelle conversation'}</button>
      )}
      {c.is_pinned && !editing && <Pin size={12} className="flex-shrink-0 text-primary" />}

      {ctx.isOpen && ctx.pos && (
        <MenuDropdown items={ctxItems} pos={ctx.pos} onClose={ctx.close} minWidth={240} />
      )}
    </div>
  )
}

// ── Section par date (conversations sans dossier) ───────────────────────────────

function DateSections({ groups, ...rest }: { groups: ReturnType<typeof groupConversations> } & ItemHandlers) {
  const sections: [string, ConversationSummary[]][] = [
    ['Épinglés', groups.pinned], ["Aujourd'hui", groups.today], ['Hier', groups.yesterday],
    ['Cette semaine', groups.week], ['Plus anciens', groups.older],
  ]
  return (
    <>
      {sections.map(([label, items]) => items.length ? (
        <div key={label} className="mb-3">
          <div className="text-xs font-semibold text-text-tertiary uppercase tracking-wide px-3 mb-1">{label}</div>
          <SortableList items={items} {...rest} />
        </div>
      ) : null)}
    </>
  )
}

interface ItemHandlers {
  activeId: string | null
  folders: JarvisFolder[]
  onSelect: (id: string) => void
  onPin: (id: string) => void
  onDelete: (id: string) => void
  onRename: (id: string, title: string) => void
  onMove: (id: string, folderId: string | null) => void
  onArchive: (id: string) => void
  onNewProject: (id: string) => void
  // New order (ids) of the section being dragged → persisted as `position`.
  onReorder: (orderedIds: string[]) => void
}
function ItemRow({ item, activeId, folders, onSelect, onPin, onDelete, onRename, onMove, onArchive, onNewProject }: { item: ConversationSummary } & ItemHandlers) {
  return (
    <ConvItem item={item} active={item.conversation.id === activeId} folders={folders}
      onSelect={() => onSelect(item.conversation.id)} onPin={() => onPin(item.conversation.id)}
      onDelete={() => onDelete(item.conversation.id)} onRename={t => onRename(item.conversation.id, t)}
      onMove={fid => onMove(item.conversation.id, fid)} onArchive={() => onArchive(item.conversation.id)}
      onNewProject={() => onNewProject(item.conversation.id)} />
  )
}

// Native HTML5 drag-and-drop reordering WITHIN a list (a date section or a
// folder). Dropping an item onto another reinserts it before that one; the new
// order is handed to onReorder (which persists positions). No external lib.
function SortableList({ items, ...rest }: { items: ConversationSummary[] } & ItemHandlers) {
  const dragId = useRef<string | null>(null)
  const [overId, setOverId] = useState<string | null>(null)
  const ids = items.map(i => i.conversation.id)

  const drop = (destId: string) => {
    const src = dragId.current
    dragId.current = null
    setOverId(null)
    if (!src || src === destId) return
    const order = ids.filter(id => id !== src)
    const di = order.indexOf(destId)
    if (di < 0) return
    order.splice(di, 0, src)
    rest.onReorder(order)
  }

  return (
    <>
      {items.map(item => {
        const id = item.conversation.id
        const showLine = overId === id && dragId.current && dragId.current !== id
        return (
          <div
            key={id}
            draggable
            onDragStart={e => { dragId.current = id; e.dataTransfer.effectAllowed = 'move' }}
            onDragOver={e => { e.preventDefault(); e.dataTransfer.dropEffect = 'move'; if (overId !== id) setOverId(id) }}
            onDragLeave={() => { if (overId === id) setOverId(null) }}
            onDrop={e => { e.preventDefault(); drop(id) }}
            onDragEnd={() => { dragId.current = null; setOverId(null) }}
            className={showLine ? 'rounded-t-lg ring-2 ring-primary/60 ring-inset' : ''}
          >
            <ItemRow item={item} {...rest} />
          </div>
        )
      })}
    </>
  )
}

// ── Dossier (repliable, avec ses conversations) ─────────────────────────────────

function FolderSection({ folder, items, collapsed, onToggle, onRenameFolder, onDeleteFolder, ...rest }: {
  folder: JarvisFolder
  items: ConversationSummary[]
  collapsed: boolean
  onToggle: () => void
  onRenameFolder: (name: string) => void
  onDeleteFolder: () => void
} & ItemHandlers) {
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(folder.name)
  const commit = () => { setEditing(false); if (draft.trim() && draft !== folder.name) onRenameFolder(draft.trim()) }
  return (
    <div className="mb-2">
      <div className="group flex items-center gap-1 px-2 py-1 rounded hover:bg-surface-2">
        <button onClick={onToggle} className="flex items-center gap-1.5 flex-1 min-w-0 text-left">
          <ChevronRight size={13} className={`transition-transform flex-shrink-0 text-text-tertiary ${collapsed ? '' : 'rotate-90'}`} />
          <Folder size={14} className="flex-shrink-0" style={{ color: folder.color ?? '#5f6368' }} />
          {editing ? (
            <input autoFocus value={draft} onChange={e => setDraft(e.target.value)} onBlur={commit}
              onClick={e => e.stopPropagation()}
              onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); commit() } else if (e.key === 'Escape') { setEditing(false); setDraft(folder.name) } }}
              className="flex-1 text-sm font-medium bg-white border border-primary rounded px-1 py-0.5 focus:outline-none" />
          ) : (
            <span className="text-sm font-medium truncate">{folder.name}</span>
          )}
          <span className="text-xs text-text-tertiary flex-shrink-0">{items.length}</span>
        </button>
        <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0">
          <span role="button" onClick={() => { setDraft(folder.name); setEditing(true) }} className="p-1 rounded hover:bg-surface-3 text-text-tertiary" title="Renommer le projet"><Pencil size={11} /></span>
          <span role="button" onClick={onDeleteFolder} className="p-1 rounded hover:bg-danger/10 text-text-tertiary hover:text-danger" title="Supprimer le projet"><Trash2 size={11} /></span>
        </div>
      </div>
      {!collapsed && (
        <div className="pl-2">
          {items.length ? <SortableList items={items} {...rest} />
            : <p className="text-xs text-text-tertiary px-3 py-1">Vide — déplacez des conversations ici</p>}
        </div>
      )}
    </div>
  )
}

// ── Main ────────────────────────────────────────────────────────────────────────

export default function JarvisSidebarBody({ collapsed = false }: { collapsed?: boolean }) {
  const {
    conversations, folders, activeConvId,
    setActiveConv, createConversation,
    togglePin, deleteConversation, archiveConversation, renameConversation,
    createFolder, renameFolder, deleteFolder, moveConversation, reorderConversations,
  } = useJarvisStore()

  const [search, setSearch] = useState('')
  const [collapsedFolders, setCollapsedFolders] = useState<Set<string>>(new Set())
  const [creatingFolder, setCreatingFolder] = useState(false)
  const [newFolderName, setNewFolderName] = useState('')

  const filtered = (search.trim()
    ? conversations.filter(c =>
        c.conversation.title?.toLowerCase().includes(search.toLowerCase()) ||
        c.last_message?.toLowerCase().includes(search.toLowerCase()))
    : conversations
  ).slice().sort(convCmp)  // manual (drag) order first, recency as tiebreak

  const ungrouped = filtered.filter(c => !c.conversation.folder_id)
  const groups = groupConversations(ungrouped)

  const handlers: ItemHandlers = {
    activeId: activeConvId, folders,
    onSelect: id => { setActiveConv(id); setSearch('') },
    onPin: id => togglePin(id),
    onDelete: id => deleteConversation(id),
    onArchive: id => archiveConversation(id),
    onRename: (id, t) => renameConversation(id, t),
    onMove: (id, fid) => moveConversation(id, fid),
    onNewProject: async id => {
      const name = await prompt({ title: 'Nouveau projet', placeholder: 'Nom du projet', confirmLabel: 'Créer' })
      if (!name) return
      const fid = await createFolder(name)
      if (fid) moveConversation(id, fid)
    },
    onReorder: ids => reorderConversations(ids),
  }

  async function handleNewConv() { const id = await createConversation(); setActiveConv(id) }
  function toggleFolder(id: string) { setCollapsedFolders(s => { const n = new Set(s); n.has(id) ? n.delete(id) : n.add(id); return n }) }
  async function submitNewFolder() {
    const n = newFolderName.trim()
    if (n) await createFolder(n)
    setNewFolderName(''); setCreatingFolder(false)
  }

  if (collapsed) {
    return (
      <nav className="flex flex-col items-center px-2 py-2 gap-1">
        <button onClick={handleNewConv} title="Nouvelle conversation"
          className="w-10 h-10 flex items-center justify-center bg-white rounded-full transition-shadow"
          style={{ boxShadow: '0 1px 3px rgba(60,64,67,0.3), 0 4px 8px rgba(60,64,67,0.15)' }}>
          <Plus size={20} className="text-text-secondary" />
        </button>
      </nav>
    )
  }

  return (
    <>
      {/* Nouvelle conversation */}
      <div className="px-3 mb-3">
        <button onClick={handleNewConv}
          className="flex items-center gap-2 bg-white text-sm font-medium text-text-primary cursor-pointer w-full hover:shadow-md transition-shadow"
          style={{ padding: '20px 25px', border: '1px solid #e0e0e0', borderRadius: '20px', boxShadow: '0 1px 3px rgba(0,0,0,0.12)' }}>
          <Plus size={20} className="text-text-secondary" />
          Nouvelle conversation
        </button>
      </div>

      {/* Quick nav */}
      <nav className="px-3 mb-1 space-y-0.5">
        <button className="w-full flex items-center gap-3 px-3 py-2 rounded-full text-sm text-text-secondary hover:bg-surface-2 transition-colors"
          onClick={() => document.getElementById('jarvis-search-input')?.focus()}>
          <Search size={18} /> Rechercher
        </button>
        <button className="w-full flex items-center gap-3 px-3 py-2 rounded-full text-sm text-text-secondary hover:bg-surface-2 transition-colors">
          <Library size={18} /> Bibliothèque
        </button>
        {/* Nouveau dossier */}
        {creatingFolder ? (
          <div className="flex items-center gap-1 px-3 py-1">
            <Folder size={16} className="text-text-tertiary flex-shrink-0" />
            <input autoFocus value={newFolderName} onChange={e => setNewFolderName(e.target.value)}
              onKeyDown={e => { if (e.key === 'Enter') submitNewFolder(); else if (e.key === 'Escape') { setCreatingFolder(false); setNewFolderName('') } }}
              placeholder="Nom du projet" className="flex-1 text-sm border border-primary rounded px-1.5 py-0.5 focus:outline-none" />
            <button onClick={submitNewFolder} className="p-1 text-success"><Check size={15} /></button>
            <button onClick={() => { setCreatingFolder(false); setNewFolderName('') }} className="p-1 text-text-tertiary"><X size={15} /></button>
          </div>
        ) : (
          <button onClick={() => setCreatingFolder(true)}
            className="w-full flex items-center gap-3 px-3 py-2 rounded-full text-sm text-text-secondary hover:bg-surface-2 transition-colors">
            <FolderPlus size={18} /> Nouveau projet
          </button>
        )}
      </nav>

      {/* Search input */}
      <div className="px-3 mb-2">
        <input id="jarvis-search-input" type="text" value={search} onChange={e => setSearch(e.target.value)}
          placeholder="Rechercher dans les discussions…"
          className="w-full text-sm border border-border rounded-full px-3 py-1.5 focus:outline-none focus:border-primary bg-surface-1 text-text-primary placeholder:text-text-tertiary"
          style={{ display: search ? 'block' : 'none' }} />
      </div>

      {/* Liste */}
      <div className="flex-1 overflow-y-auto px-1 pb-4">
        {search ? (
          <>
            <div className="mb-2 px-3 text-xs font-semibold text-text-tertiary uppercase tracking-wide">Résultats</div>
            {filtered.map(item => <ItemRow key={item.conversation.id} item={item} {...handlers} />)}
          </>
        ) : (
          <>
            {/* Dossiers */}
            {folders.map(f => (
              <FolderSection key={f.id} folder={f}
                items={conversations.filter(c => c.conversation.folder_id === f.id).slice().sort(convCmp)}
                collapsed={collapsedFolders.has(f.id)} onToggle={() => toggleFolder(f.id)}
                onRenameFolder={n => renameFolder(f.id, n)} onDeleteFolder={() => deleteFolder(f.id)}
                {...handlers} />
            ))}
            {/* Conversations sans dossier (par date) */}
            <DateSections groups={groups} {...handlers} />
          </>
        )}

        {!conversations.length && (
          <p className="text-xs text-text-tertiary text-center mt-6 px-3">Aucune conversation pour l'instant</p>
        )}
      </div>
    </>
  )
}
