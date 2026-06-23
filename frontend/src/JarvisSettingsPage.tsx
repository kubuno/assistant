import React, { useState, useEffect } from 'react'
import {
  ArrowLeft, Sparkles, Bot,
  Plus, Pencil, Trash2, Save, CheckCircle, XCircle, Check,
  Eye, EyeOff, AlertCircle, ExternalLink,
} from 'lucide-react'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useAuthStore } from '@kubuno/sdk'
import { jarvisApi, type JarvisAgent, type ProviderConfig, type UpdateProviderDto } from './api'
import { Toggle, Button, Input, Textarea, Radio } from '@ui'
import { useModulePrefs } from './userPrefs'

// ── Per-user preferences (backend, cross-device via core users.preferences) ─────

interface JarvisPrefs {
  responseStyle:  string   // 'concise' | 'balanced' | 'detailed'
  sendOnEnter:    boolean  // Enter sends (Shift+Enter = newline) vs the inverse
  streaming:      boolean  // stream the answer token by token
  showTimestamps: boolean  // show the time under each message
  showTokens:     boolean  // show token usage under assistant replies
  bubbleTheme:    string   // 'default' | 'compact' | 'comfortable'
}

const DEFAULT_PREFS: JarvisPrefs = {
  responseStyle: 'balanced', sendOnEnter: true, streaming: true,
  showTimestamps: false, showTokens: true, bubbleTheme: 'default',
}

// ── Mail-style layout helpers ───────────────────────────────────────────────────

function SettingsRow({ label, description, children }: {
  label: string; description?: string; children: React.ReactNode
}) {
  return (
    <div className="flex items-start gap-8 py-4 border-b border-[#e8eaed] last:border-0">
      <div className="w-60 flex-shrink-0">
        <p className="text-sm text-[#202124] font-normal">{label}</p>
        {description && <p className="text-xs text-text-tertiary mt-0.5 leading-relaxed">{description}</p>}
      </div>
      <div className="flex-1">{children}</div>
    </div>
  )
}

function RadioGroup({ options, value, onChange }: {
  options: { value: string; label: string }[]; value: string; onChange: (v: string) => void
}) {
  return (
    <div className="flex flex-col items-start gap-2">
      {options.map(opt => (
        <Radio key={opt.value} checked={value === opt.value} onChange={() => onChange(opt.value)} label={opt.label} />
      ))}
    </div>
  )
}

// ── Préférences tab (per-user) ──────────────────────────────────────────────────

function PreferencesTab() {
  const { t } = useTranslation('jarvis')
  const { prefs: saved, update } = useModulePrefs<JarvisPrefs>('jarvis', DEFAULT_PREFS)
  const [prefs, setPrefs] = useState<JarvisPrefs>(saved)
  const [savedFlag, setSavedFlag] = useState(false)
  const [busy, setBusy] = useState(false)

  const set = <K extends keyof JarvisPrefs>(key: K, value: JarvisPrefs[K]) =>
    setPrefs(p => ({ ...p, [key]: value }))

  const save = async () => {
    setBusy(true)
    try {
      await update(prefs)
      setSavedFlag(true)
      setTimeout(() => setSavedFlag(false), 2500)
    } finally { setBusy(false) }
  }

  return (
    <div>
      <SettingsRow
        label={t('jarvis_pref_style', { defaultValue: 'Style des réponses' })}
        description={t('jarvis_pref_style_desc', { defaultValue: 'Longueur et niveau de détail attendus de l\'assistant.' })}
      >
        <RadioGroup
          value={prefs.responseStyle}
          onChange={v => set('responseStyle', v)}
          options={[
            { value: 'concise',  label: t('jarvis_pref_style_concise',  { defaultValue: 'Concis (réponses courtes)' }) },
            { value: 'balanced', label: t('jarvis_pref_style_balanced', { defaultValue: 'Équilibré' }) },
            { value: 'detailed', label: t('jarvis_pref_style_detailed', { defaultValue: 'Détaillé (explications complètes)' }) },
          ]}
        />
      </SettingsRow>

      <SettingsRow
        label={t('jarvis_pref_bubble', { defaultValue: 'Densité de la conversation' })}
        description={t('jarvis_pref_bubble_desc', { defaultValue: 'Espacement entre les messages.' })}
      >
        <RadioGroup
          value={prefs.bubbleTheme}
          onChange={v => set('bubbleTheme', v)}
          options={[
            { value: 'compact',     label: t('jarvis_pref_bubble_compact',     { defaultValue: 'Compacte' }) },
            { value: 'default',     label: t('jarvis_pref_bubble_default',     { defaultValue: 'Normale' }) },
            { value: 'comfortable', label: t('jarvis_pref_bubble_comfortable', { defaultValue: 'Confortable' }) },
          ]}
        />
      </SettingsRow>

      <SettingsRow
        label={t('jarvis_pref_send', { defaultValue: 'Envoi du message' })}
        description={t('jarvis_pref_send_desc', { defaultValue: 'Touche Entrée pour envoyer ; Maj+Entrée insère un saut de ligne.' })}
      >
        <label className="flex items-center gap-2 cursor-pointer select-none">
          <Toggle checked={prefs.sendOnEnter} onChange={() => set('sendOnEnter', !prefs.sendOnEnter)} />
          <span className="text-sm text-text-primary">{t('jarvis_pref_send_on', { defaultValue: 'Envoyer avec la touche Entrée' })}</span>
        </label>
      </SettingsRow>

      <SettingsRow
        label={t('jarvis_pref_streaming', { defaultValue: 'Réponses en continu' })}
        description={t('jarvis_pref_streaming_desc', { defaultValue: 'Afficher la réponse au fur et à mesure de sa génération.' })}
      >
        <label className="flex items-center gap-2 cursor-pointer select-none">
          <Toggle checked={prefs.streaming} onChange={() => set('streaming', !prefs.streaming)} />
          <span className="text-sm text-text-primary">{t('jarvis_pref_streaming_on', { defaultValue: 'Activer le streaming' })}</span>
        </label>
      </SettingsRow>

      <SettingsRow label={t('jarvis_pref_timestamps', { defaultValue: 'Horodatage' })}>
        <label className="flex items-center gap-2 cursor-pointer select-none">
          <Toggle checked={prefs.showTimestamps} onChange={() => set('showTimestamps', !prefs.showTimestamps)} />
          <span className="text-sm text-text-primary">{t('jarvis_pref_timestamps_on', { defaultValue: 'Afficher l\'heure sous chaque message' })}</span>
        </label>
      </SettingsRow>

      <SettingsRow label={t('jarvis_pref_tokens', { defaultValue: 'Jetons (tokens)' })}>
        <label className="flex items-center gap-2 cursor-pointer select-none">
          <Toggle checked={prefs.showTokens} onChange={() => set('showTokens', !prefs.showTokens)} />
          <span className="text-sm text-text-primary">{t('jarvis_pref_tokens_on', { defaultValue: 'Afficher l\'utilisation des jetons sous les réponses' })}</span>
        </label>
      </SettingsRow>

      <div className="pt-5 flex items-center gap-3">
        <Button onClick={save} loading={busy}>
          {savedFlag
            ? <><Check size={14} className="mr-1.5 inline" />{t('jarvis_settings_saved', { defaultValue: 'Enregistré' })}</>
            : t('jarvis_settings_save_changes', { defaultValue: 'Enregistrer les modifications' })}
        </Button>
        <Button variant="ghost" onClick={() => setPrefs(saved)}>
          {t('common_cancel', { defaultValue: 'Annuler' })}
        </Button>
      </div>
    </div>
  )
}

// ── Admin-only providers tab (instance-wide secrets: API keys / endpoints) ──────

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

// ── Agents tab (per-user custom agents) ──────────────────────────────────────────

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

      {/* Custom agents */}
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

      {/* System agents (read-only) */}
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

// ── About tab ───────────────────────────────────────────────────────────────────

function AboutTab() {
  const { t } = useTranslation('jarvis')
  return (
    <div className="rounded-xl border border-border overflow-hidden">
      <div className="flex items-center gap-3 px-5 py-4 border-b border-border bg-surface-1">
        <div className="w-10 h-10 rounded-xl bg-violet-100 flex items-center justify-center shrink-0">
          <Sparkles size={20} className="text-violet-600" />
        </div>
        <div>
          <p className="text-sm font-semibold text-text-primary">Kubuno Jarvis</p>
          <p className="text-xs text-text-tertiary">v0.1.0 · {t('jarvis_official_module', { defaultValue: 'Module officiel' })}</p>
        </div>
        <span className="ml-auto text-xs font-medium px-2 py-0.5 rounded-full bg-orange-100 text-orange-700">Rust</span>
      </div>
      <div className="px-5 py-4">
        <a href="https://github.com/kubuno/jarvis" target="_blank" rel="noopener noreferrer"
          className="inline-flex items-center gap-1.5 text-sm text-primary hover:underline">
          <ExternalLink size={13} /> github.com/kubuno/jarvis
        </a>
      </div>
    </div>
  )
}

// ── Main page (mail-style breadcrumb + tab bar) ─────────────────────────────────

type Tab = 'preferences' | 'agents' | 'providers' | 'about'

export default function JarvisSettingsPage() {
  const { t } = useTranslation('jarvis')
  const isAdmin = useAuthStore(s => s.user?.role === 'admin')
  const [tab, setTab] = useState<Tab>('preferences')

  // The providers tab holds instance-wide secrets (API keys) → admin-only.
  const tabs: { id: Tab; label: string; adminOnly?: boolean }[] = [
    { id: 'preferences', label: t('jarvis_tab_preferences', { defaultValue: 'Préférences' }) },
    { id: 'agents',      label: t('jarvis_tab_agents', { defaultValue: 'Agents' }) },
    { id: 'providers',   label: t('jarvis_tab_providers', { defaultValue: 'Fournisseurs' }), adminOnly: true },
    { id: 'about',       label: t('jarvis_tab_about', { defaultValue: 'À propos' }) },
  ]
  const visibleTabs = tabs.filter(tb => !tb.adminOnly || isAdmin)

  return (
    <div className="flex flex-col h-full bg-white overflow-hidden">
      {/* Breadcrumb header */}
      <div className="flex items-center gap-2 px-6 py-2.5 border-b border-[#e8eaed] flex-shrink-0" style={{ background: '#f8f9fa' }}>
        <Link to="/jarvis" className="flex items-center gap-1.5 text-sm text-[#1a73e8] hover:underline">
          <ArrowLeft size={14} />
          Jarvis
        </Link>
        <span className="text-text-tertiary text-sm">/</span>
        <div className="flex items-center gap-1.5">
          <Sparkles size={15} className="text-text-secondary" />
          <span className="text-sm text-text-primary">{t('jarvis_settings_title', { defaultValue: 'Réglages' })}</span>
        </div>
      </div>

      {/* Tab bar (Gmail-style) */}
      <div className="flex items-end border-b border-[#e8eaed] px-4 flex-shrink-0 overflow-x-auto" style={{ background: '#fff' }}>
        {visibleTabs.map(tb => (
          <button key={tb.id} onClick={() => setTab(tb.id)}
            className={`px-4 py-3 text-sm border-b-2 -mb-px transition-colors whitespace-nowrap ${
              tab === tb.id ? 'border-[#1a73e8] text-[#1a73e8] font-medium' : 'border-transparent text-[#5f6368] hover:text-[#202124] hover:bg-[#f1f3f4]'}`}>
            {tb.label}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-3xl mx-auto px-8 py-6">
          {tab === 'preferences' && <PreferencesTab />}
          {tab === 'agents'      && <AgentsTab />}
          {tab === 'providers'   && isAdmin && <ProvidersTab />}
          {tab === 'about'       && <AboutTab />}
        </div>
      </div>
    </div>
  )
}
