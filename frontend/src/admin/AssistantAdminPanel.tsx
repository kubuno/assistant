// Instance administration of the assistant, rendered in the core admin console
// under Modules ▸ Assistant. Registered as a CUSTOM section rather than left to
// the generic settings form because the values here are SECRETS: the core's
// settings store answers in clear to anything holding the internal secret, so
// provider credentials live in the module's own table, behind the module's own
// admin guard, and are read back masked.
//
// The scalar policies (models, limits, retention) are declared in module.toml
// and rendered by the core's generic form on the same pages; this section only
// covers what a generated form cannot express — a credential that is written
// but never read back.
//
// This surface used to sit in the per-user settings page behind an `adminOnly`
// tab, where a personal preferences screen was answering questions about the
// whole instance. It moved here whole.

import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AlertCircle, CheckCircle, Eye, EyeOff, Save } from 'lucide-react'
import { Button, Input, Toggle } from '@ui'
import { ModuleAdminRegistry } from '@kubuno/sdk'
import { assistantApi, type ProviderConfig, type UpdateProviderDto } from '../api'

/** Display identity of each supported engine. Unknown ids degrade gracefully. */
const PROVIDER_META: Record<string, { name: string; icon: string }> = {
  ollama:    { name: 'Ollama',    icon: '🦙' },
  openai:    { name: 'OpenAI',    icon: '⚡' },
  anthropic: { name: 'Anthropic', icon: '🤖' },
}

/** The local engine needs no credential — its card hides the key field. */
const LOCAL_PROVIDER = 'ollama'

function ProviderCard({
  config, onUpdate,
}: {
  config: ProviderConfig
  onUpdate: (provider: string, dto: UpdateProviderDto) => Promise<void>
}) {
  const { t } = useTranslation('assistant')
  const [showKey,  setShowKey]  = useState(false)
  const [apiKey,   setApiKey]   = useState('')
  const [baseUrl,  setBaseUrl]  = useState(config.base_url)
  const [defModel, setDefModel] = useState(config.default_model)
  const [enabled,  setEnabled]  = useState(config.enabled)
  const [saving,   setSaving]   = useState(false)
  const [saved,    setSaved]    = useState(false)
  const [err,      setErr]      = useState<string | null>(null)

  const meta = PROVIDER_META[config.provider] ?? { name: config.provider, icon: '🔧' }
  const isLocal = config.provider === LOCAL_PROVIDER

  async function save() {
    setSaving(true); setErr(null)
    try {
      const dto: UpdateProviderDto = { enabled, base_url: baseUrl.trim(), default_model: defModel.trim() }
      // An untouched field must not overwrite the stored key: only send it when
      // the administrator actually typed something.
      if (apiKey) dto.api_key = apiKey
      await onUpdate(config.provider, dto)
      setApiKey('')
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } catch (e) {
      // The backend refuses a malformed address; show its reason rather than a
      // generic failure, because the administrator can act on it.
      const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message
      setErr(msg || t('assistant_save_error'))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="border border-border rounded-xl p-5 bg-surface-1">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <span className="text-xl leading-none">{meta.icon}</span>
          <div>
            <p className="text-sm text-text-primary">{meta.name}</p>
            <p className="text-xs text-text-tertiary">
              {isLocal
                ? t('assistant_admin_local_engine')
                : config.has_api_key
                  ? t('assistant_admin_key_on_file', { preview: config.api_key })
                  : t('assistant_admin_no_key')}
            </p>
          </div>
        </div>
        <Toggle checked={enabled} onChange={e => setEnabled(e.target.checked)} size="sm" />
      </div>

      <div className="space-y-3">
        {!isLocal && (
          <div>
            <label className="text-xs text-text-secondary block mb-1">{t('assistant_api_key')}</label>
            <div className="relative">
              <input
                type={showKey ? 'text' : 'password'}
                value={apiKey}
                onChange={e => setApiKey(e.target.value)}
                placeholder={config.has_api_key ? config.api_key : t('assistant_api_key_placeholder')}
                autoComplete="off"
                className="w-full pr-9 pl-3 py-2 text-sm border border-border rounded-lg
                           bg-surface-2 focus:outline-none focus:border-primary"
              />
              <button
                type="button"
                onClick={() => setShowKey(v => !v)}
                aria-label={showKey ? t('assistant_admin_hide_key') : t('assistant_admin_show_key')}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-text-tertiary hover:text-text-primary"
              >
                {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
              </button>
            </div>
            <p className="mt-1 text-xs text-text-tertiary">{t('assistant_admin_key_help')}</p>
          </div>
        )}

        <Input
          label={t('assistant_base_url')}
          type="text"
          value={baseUrl}
          onChange={e => setBaseUrl(e.target.value)}
        />

        <Input
          label={t('assistant_default_model')}
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
          {saved ? t('assistant_saved') : saving ? t('assistant_saving') : t('common_save')}
        </Button>
      </div>
    </div>
  )
}

function ProvidersSection() {
  const { t } = useTranslation('assistant')
  const [providers, setProviders] = useState<ProviderConfig[]>([])
  const [loading,   setLoading]   = useState(true)
  const [forbidden, setForbidden] = useState(false)
  const [error,     setError]     = useState<string | null>(null)

  useEffect(() => {
    assistantApi.listProviders()
      .then(setProviders)
      .catch((e: { response?: { status?: number } }) => {
        if (e?.response?.status === 403) setForbidden(true)
        else setError(t('assistant_providers_load_error'))
      })
      .finally(() => setLoading(false))
  }, [t])

  async function handleUpdate(provider: string, dto: UpdateProviderDto) {
    const updated = await assistantApi.updateProvider(provider, dto)
    setProviders(prev => prev.map(p => (p.provider === provider ? { ...p, ...updated } : p)))
  }

  if (loading) return null
  if (forbidden) {
    return (
      <div className="rounded-xl border border-border px-5 py-6 text-sm text-text-tertiary">
        {t('assistant_admin_forbidden')}
      </div>
    )
  }
  if (error) {
    return (
      <p className="text-sm text-danger flex items-center gap-2">
        <AlertCircle size={16} /> {error}
      </p>
    )
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-text-secondary">{t('assistant_admin_providers_help')}</p>
      <div className="grid gap-4">
        {providers.map(p => (
          <ProviderCard key={p.provider} config={p} onUpdate={handleUpdate} />
        ))}
      </div>
    </div>
  )
}

/** Called once by the module's `register()`. */
export function registerAssistantAdmin() {
  ModuleAdminRegistry.register({
    moduleId:  'assistant',
    id:        'providers',
    group:     'availability',
    labelKey:  'assistant:assistant_admin_providers_tab',
    icon:      'KeyRound',
    position:  10,
    Component: ProvidersSection,
  })
}
