import { useState, useEffect } from 'react'
import {
  ChevronLeft, Sparkles, Settings2, Bot,
  Plus, Pencil, Trash2, Save, CheckCircle, XCircle,
  Eye, EyeOff, AlertCircle,
} from 'lucide-react'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { jarvisApi, type JarvisAgent, type ProviderConfig, type UpdateProviderDto } from './api'
import { Toggle, Button, Tabs, Input, Textarea } from '@ui'
import type { TabDef } from '@ui'

type Tab = 'providers' | 'agents'

// ── Providers tab ──────────────────────────────────────────────────────────────

const PROVIDER_META: Record<string, { name: string; colorClass: string; icon: string }> = {
  ollama:    { name: 'Ollama',        colorClass: 'bg-purple-100 text-purple-700',   icon: '🦙' },
  openai:    { name: 'OpenAI',        colorClass: 'bg-emerald-100 text-emerald-700', icon: '⚡' },
  anthropic: { name: 'Anthropic',     colorClass: 'bg-orange-100 text-orange-700',   icon: '🤖' },
  google:    { name: 'Google Gemini', colorClass: 'bg-blue-100 text-blue-700',       icon: '✦' },
}

function ProviderCard({ config, onUpdate }: { config: ProviderConfig; onUpdate: (p: string, d: UpdateProviderDto) => Promise<void> }) {
  const { t } = useTranslation('jarvis')
  const [showKey,   setShowKey]   = useState(false)
  const [apiKey,    setApiKey]    = useState('')
  const [baseUrl,   setBaseUrl]   = useState(config.base_url)
  const [defModel,  setDefModel]  = useState(config.default_model)
  const [enabled,   setEnabled]   = useState(config.enabled)
  const [saving,    setSaving]    = useState(false)
  const [saved,     setSaved]     = useState(false)
  const [err,       setErr]       = useState<string | null>(null)

  const meta = PROVIDER_META[config.provider] ?? { name: config.provider, colorClass: 'bg-surface-2 text-text-secondary', icon: '🔧' }

  async function save() {
    setSaving(true); setErr(null)
    try {
      const dto: UpdateProviderDto = { enabled, base_url: baseUrl, default_model: defModel }
      if (apiKey) dto.api_key = apiKey
      await onUpdate(config.provider, dto)
      setApiKey('')
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } catch {
      setErr(t('jarvis_save_error'))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="border border-border rounded-xl p-5 bg-white">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <span className="text-xl leading-none">{meta.icon}</span>
          <div>
            <p className="font-semibold text-sm text-text-primary">{meta.name}</p>
            <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${meta.colorClass}`}>
              {config.provider}
            </span>
          </div>
        </div>
        <Toggle checked={enabled} onChange={e => setEnabled(e.target.checked)} size="sm" />
      </div>

      <div className="space-y-3">
        {config.provider !== 'ollama' && (
          <div>
            <label className="text-xs font-medium text-text-secondary block mb-1">{t('jarvis_api_key')}</label>
            <div className="relative">
              <input
                type={showKey ? 'text' : 'password'}
                value={apiKey}
                onChange={e => setApiKey(e.target.value)}
                placeholder={config.api_key || t('jarvis_api_key_placeholder')}
                className="w-full pr-9 pl-3 py-2 text-sm border border-border rounded-lg
                           bg-surface-1 focus:outline-none focus:border-primary"
              />
              <button
                type="button"
                onClick={() => setShowKey(v => !v)}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-text-tertiary hover:text-text-primary"
              >
                {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
              </button>
            </div>
          </div>
        )}

        <Input
          label={t('jarvis_base_url')}
          type="text"
          value={baseUrl}
          onChange={e => setBaseUrl(e.target.value)}
        />

        <Input
          label={t('jarvis_default_model')}
          type="text"
          value={defModel}
          onChange={e => setDefModel(e.target.value)}
        />
      </div>

      {err && (
        <p className="mt-2 text-xs text-danger flex items-center gap-1">
          <AlertCircle size={12} /> {err}
        </p>
      )}

      <div className="mt-4 flex justify-end">
        <Button onClick={save} disabled={saving}>
          {saved ? <CheckCircle size={14} /> : <Save size={14} />}
          {saved ? t('jarvis_saved') : saving ? t('jarvis_saving') : t('common_save')}
        </Button>
      </div>
    </div>
  )
}

function ProvidersTab() {
  const { t } = useTranslation('jarvis')
  const [providers, setProviders] = useState<ProviderConfig[]>([])
  const [loading,   setLoading]   = useState(true)
  const [error,     setError]     = useState<string | null>(null)

  useEffect(() => {
    jarvisApi.listProviders()
      .then(setProviders)
      .catch(() => setError(t('jarvis_providers_load_error')))
      .finally(() => setLoading(false))
  }, [t])

  async function handleUpdate(provider: string, dto: UpdateProviderDto) {
    const updated = await jarvisApi.updateProvider(provider, dto)
    setProviders(prev => prev.map(p => p.provider === provider ? { ...p, ...updated } : p))
  }

  if (loading) return <div className="py-8 text-center text-sm text-text-tertiary">{t('common_loading')}</div>
  if (error)   return (
    <div className="flex items-center gap-2 text-danger text-sm p-4">
      <XCircle size={16} /> {error}
    </div>
  )

  return (
    <div className="space-y-4">
      <p className="text-sm text-text-secondary">
        {t('jarvis_providers_help')}
      </p>
      <div className="grid gap-4">
        {providers.map(p => (
          <ProviderCard key={p.provider} config={p} onUpdate={handleUpdate} />
        ))}
      </div>
    </div>
  )
}

// ── Agents tab ─────────────────────────────────────────────────────────────────

interface AgentFormData {
  name:          string
  description:   string
  system_prompt: string
  default_model: string
}

function AgentForm({
  initial,
  onSave,
  onCancel,
}: {
  initial?: Partial<AgentFormData>
  onSave:   (d: AgentFormData) => Promise<void>
  onCancel: () => void
}) {
  const { t } = useTranslation('jarvis')
  const [name,         setName]         = useState(initial?.name          ?? '')
  const [description,  setDescription]  = useState(initial?.description   ?? '')
  const [systemPrompt, setSystemPrompt] = useState(initial?.system_prompt ?? '')
  const [defModel,     setDefModel]     = useState(initial?.default_model ?? '')
  const [saving,       setSaving]       = useState(false)
  const [err,          setErr]          = useState<string | null>(null)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    if (!name.trim()) { setErr(t('jarvis_name_required')); return }
    setSaving(true); setErr(null)
    try {
      await onSave({ name: name.trim(), description: description.trim(), system_prompt: systemPrompt, default_model: defModel.trim() })
    } catch {
      setErr(t('jarvis_save_error'))
      setSaving(false)
    }
  }

  return (
    <form onSubmit={submit} className="border border-border rounded-xl p-5 bg-white space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <Input
          label={t('jarvis_agent_name')}
          autoFocus
          value={name}
          onChange={e => setName(e.target.value)}
          placeholder={t('jarvis_agent_name_placeholder')}
          maxLength={100}
        />
        <Input
          label={t('jarvis_preferred_model')}
          value={defModel}
          onChange={e => setDefModel(e.target.value)}
          placeholder={t('jarvis_preferred_model_placeholder')}
        />
      </div>

      <Input
        label={t('jarvis_description')}
        value={description}
        onChange={e => setDescription(e.target.value)}
        placeholder={t('jarvis_description_placeholder')}
        maxLength={255}
      />

      <Textarea
        label={t('jarvis_system_prompt')}
        value={systemPrompt}
        onChange={e => setSystemPrompt(e.target.value)}
        rows={5}
        placeholder={t('jarvis_system_prompt_placeholder')}
        className="h-auto min-h-0 resize-y"
      />

      {err && (
        <p className="text-xs text-danger flex items-center gap-1">
          <AlertCircle size={12} /> {err}
        </p>
      )}

      <div className="flex gap-2 justify-end pt-1">
        <Button type="button" variant="ghost" onClick={onCancel}>{t('common_cancel')}</Button>
        <Button type="submit" icon={<Save size={14} />} disabled={!name.trim()} loading={saving}>
          {saving ? t('jarvis_saving') : t('common_save')}
        </Button>
      </div>
    </form>
  )
}

function AgentsTab() {
  const { t } = useTranslation('jarvis')
  const [agents,   setAgents]   = useState<JarvisAgent[]>([])
  const [loading,  setLoading]  = useState(true)
  const [error,    setError]    = useState<string | null>(null)
  const [creating, setCreating] = useState(false)
  const [editing,  setEditing]  = useState<string | null>(null)
  const [deleting, setDeleting] = useState<string | null>(null)

  useEffect(() => {
    jarvisApi.listAgents()
      .then(setAgents)
      .catch(() => setError(t('jarvis_agents_load_error')))
      .finally(() => setLoading(false))
  }, [t])

  async function handleCreate(data: AgentFormData) {
    const created = await jarvisApi.createAgent(data)
    setAgents(prev => [...prev, created])
    setCreating(false)
  }

  async function handleEdit(id: string, data: AgentFormData) {
    const updated = await jarvisApi.updateAgent(id, data)
    setAgents(prev => prev.map(a => a.id === id ? updated : a))
    setEditing(null)
  }

  async function handleDelete(id: string) {
    setDeleting(id)
    try {
      await jarvisApi.deleteAgent(id)
      setAgents(prev => prev.filter(a => a.id !== id))
    } finally {
      setDeleting(null)
    }
  }

  if (loading) return <div className="py-8 text-center text-sm text-text-tertiary">{t('common_loading')}</div>
  if (error)   return (
    <div className="flex items-center gap-2 text-danger text-sm p-4">
      <XCircle size={16} /> {error}
    </div>
  )

  const systemAgents = agents.filter(a => a.is_system)
  const userAgents   = agents.filter(a => !a.is_system)

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <p className="text-sm text-text-secondary">
          {t('jarvis_agents_help')}
        </p>
        {!creating && (
          <Button size="sm" icon={<Plus size={14} />} onClick={() => setCreating(true)}>
            {t('jarvis_new_agent')}
          </Button>
        )}
      </div>

      {creating && (
        <AgentForm
          onSave={handleCreate}
          onCancel={() => setCreating(false)}
        />
      )}

      {/* Agents personnalisés */}
      {userAgents.length === 0 && !creating ? (
        <div className="border border-dashed border-border rounded-xl p-8 text-center">
          <Bot size={32} className="mx-auto text-text-tertiary mb-2" />
          <p className="text-sm text-text-secondary">{t('jarvis_no_custom_agents')}</p>
          <p className="text-xs text-text-tertiary mt-1">{t('jarvis_no_custom_agents_hint')}</p>
        </div>
      ) : (
        <div className="space-y-3">
          {userAgents.map(agent => (
            editing === agent.id ? (
              <AgentForm
                key={agent.id}
                initial={{
                  name:          agent.name,
                  description:   agent.description ?? '',
                  system_prompt: agent.system_prompt,
                  default_model: agent.default_model ?? '',
                }}
                onSave={d => handleEdit(agent.id, d)}
                onCancel={() => setEditing(null)}
              />
            ) : (
              <div key={agent.id} className="border border-border rounded-xl p-4 bg-white flex items-start gap-3">
                <div className="w-8 h-8 rounded-lg bg-primary-light flex items-center justify-center flex-shrink-0 mt-0.5">
                  <Bot size={16} className="text-primary" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-text-primary">{agent.name}</p>
                  {agent.description && (
                    <p className="text-xs text-text-secondary mt-0.5">{agent.description}</p>
                  )}
                  {agent.default_model && (
                    <p className="text-xs text-text-tertiary mt-1 font-mono">{agent.default_model}</p>
                  )}
                  {agent.system_prompt && (
                    <p className="text-xs text-text-tertiary mt-1 line-clamp-2 italic">
                      {agent.system_prompt}
                    </p>
                  )}
                </div>
                <div className="flex items-center gap-1 flex-shrink-0">
                  <button
                    onClick={() => setEditing(agent.id)}
                    className="p-1.5 rounded-lg text-text-tertiary hover:text-text-primary hover:bg-surface-2"
                    title={t('common_edit')}
                  >
                    <Pencil size={14} />
                  </button>
                  <button
                    onClick={() => handleDelete(agent.id)}
                    disabled={deleting === agent.id}
                    className="p-1.5 rounded-lg text-text-tertiary hover:text-danger hover:bg-danger-light disabled:opacity-50"
                    title={t('common_delete')}
                  >
                    {deleting === agent.id ? (
                      <span className="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin inline-block" />
                    ) : (
                      <Trash2 size={14} />
                    )}
                  </button>
                </div>
              </div>
            )
          ))}
        </div>
      )}

      {/* Agents système (lecture seule) */}
      {systemAgents.length > 0 && (
        <div>
          <p className="text-xs font-medium text-text-tertiary uppercase tracking-wide mb-2">
            {t('jarvis_system_agents_title')}
          </p>
          <div className="space-y-2">
            {systemAgents.map(agent => (
              <div key={agent.id} className="border border-border rounded-xl p-4 bg-surface-1 flex items-start gap-3">
                <div className="w-8 h-8 rounded-lg bg-surface-2 flex items-center justify-center flex-shrink-0 mt-0.5">
                  <Bot size={16} className="text-text-secondary" />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <p className="text-sm font-medium text-text-primary">{agent.name}</p>
                    <span className="text-xs bg-surface-3 text-text-tertiary px-1.5 py-0.5 rounded">{t('jarvis_system_badge')}</span>
                  </div>
                  {agent.description && (
                    <p className="text-xs text-text-secondary mt-0.5">{agent.description}</p>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

// ── Page principale ────────────────────────────────────────────────────────────

export default function JarvisSettingsPage() {
  const { t } = useTranslation('jarvis')
  const [tab, setTab] = useState<Tab>('providers')

  const TABS: TabDef<Tab>[] = [
    { id: 'providers', label: t('jarvis_tab_providers'), icon: Settings2 },
    { id: 'agents',    label: t('jarvis_tab_agents'),    icon: Bot },
  ]

  return (
    <div className="flex-1 overflow-y-auto bg-surface-1">
      <div className="max-w-3xl mx-auto px-6 py-8">

        {/* Header */}
        <div className="flex items-center gap-3 mb-8">
          <Link
            to="/jarvis"
            className="p-1.5 rounded-lg text-text-tertiary hover:text-text-primary hover:bg-surface-2 transition-colors"
          >
            <ChevronLeft size={18} />
          </Link>
          <div className="w-9 h-9 rounded-xl bg-primary-light flex items-center justify-center">
            <Sparkles size={18} className="text-primary" />
          </div>
          <div>
            <h1 className="text-lg font-semibold text-text-primary">{t('jarvis_settings_title')}</h1>
            <p className="text-xs text-text-tertiary">{t('jarvis_settings_subtitle')}</p>
          </div>
        </div>

        <Tabs tabs={TABS} value={tab} onChange={setTab} className="mb-6" />

        {/* Content */}
        {tab === 'providers' && <ProvidersTab />}
        {tab === 'agents'    && <AgentsTab />}

      </div>
    </div>
  )
}
