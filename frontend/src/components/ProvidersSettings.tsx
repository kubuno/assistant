import { useState, useEffect } from 'react'
import { Settings2, Eye, EyeOff, Save, CheckCircle, XCircle } from 'lucide-react'
import { jarvisApi, ProviderConfig, UpdateProviderDto } from '../api'
import { Toggle, Button, Input } from '@ui'

const PROVIDER_LABELS: Record<string, { name: string; color: string; icon: string }> = {
  ollama:    { name: 'Ollama',          color: 'bg-purple-100 text-purple-700',  icon: '🦙' },
  openai:    { name: 'OpenAI',          color: 'bg-emerald-100 text-emerald-700', icon: '⚡' },
  anthropic: { name: 'Anthropic',       color: 'bg-orange-100 text-orange-700',  icon: '🤖' },
  google:    { name: 'Google Gemini',   color: 'bg-blue-100 text-blue-700',      icon: '✦' },
}

interface ProviderCardProps {
  config:   ProviderConfig
  onUpdate: (provider: string, data: UpdateProviderDto) => Promise<void>
}

function ProviderCard({ config, onUpdate }: ProviderCardProps) {
  const [showKey, setShowKey]     = useState(false)
  const [apiKey, setApiKey]       = useState('')
  const [baseUrl, setBaseUrl]     = useState(config.base_url)
  const [defModel, setDefModel]   = useState(config.default_model)
  const [enabled, setEnabled]     = useState(config.enabled)
  const [saving, setSaving]       = useState(false)
  const [saved, setSaved]         = useState(false)

  const meta = PROVIDER_LABELS[config.provider] ?? { name: config.provider, color: 'bg-gray-100 text-gray-700', icon: '🔧' }

  async function save() {
    setSaving(true)
    try {
      const dto: UpdateProviderDto = { enabled, base_url: baseUrl, default_model: defModel }
      if (apiKey) dto.api_key = apiKey
      await onUpdate(config.provider, dto)
      setApiKey('')
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="border border-border rounded-xl p-5 bg-surface-0">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <span className="text-xl">{meta.icon}</span>
          <div>
            <span className="font-semibold text-text-primary">{meta.name}</span>
            <span className={`ml-2 text-xs px-2 py-0.5 rounded-full font-medium ${meta.color}`}>
              {config.provider}
            </span>
          </div>
        </div>
        <Toggle checked={enabled} onChange={e => setEnabled(e.target.checked)} size="sm" />
      </div>

      <div className="space-y-3">
        {config.provider !== 'ollama' && (
          <div>
            <label className="text-xs font-medium text-text-secondary block mb-1">Clé API</label>
            <div className="relative">
              <input
                type={showKey ? 'text' : 'password'}
                value={apiKey}
                onChange={e => setApiKey(e.target.value)}
                placeholder={config.api_key ? config.api_key : 'Entrez votre clé API…'}
                className="w-full pr-9 pl-3 py-2 text-sm border border-border rounded-lg bg-surface-1 focus:outline-none focus:border-primary"
              />
              <button
                type="button"
                onClick={() => setShowKey(v => !v)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-text-tertiary hover:text-text-primary"
              >
                {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
              </button>
            </div>
          </div>
        )}

        <Input
          label="URL de base"
          type="text"
          value={baseUrl}
          onChange={e => setBaseUrl(e.target.value)}
        />

        <Input
          label="Modèle par défaut"
          type="text"
          value={defModel}
          onChange={e => setDefModel(e.target.value)}
        />
      </div>

      <div className="mt-4 flex justify-end">
        <Button onClick={save} disabled={saving}>
          {saved ? <CheckCircle size={14} /> : saving ? null : <Save size={14} />}
          {saved ? 'Enregistré' : saving ? 'Enregistrement…' : 'Enregistrer'}
        </Button>
      </div>
    </div>
  )
}

export default function ProvidersSettings() {
  const [providers, setProviders] = useState<ProviderConfig[]>([])
  const [loading, setLoading]     = useState(true)
  const [error, setError]         = useState<string | null>(null)

  useEffect(() => {
    jarvisApi.listProviders()
      .then(setProviders)
      .catch(() => setError('Impossible de charger les fournisseurs'))
      .finally(() => setLoading(false))
  }, [])

  async function handleUpdate(provider: string, dto: UpdateProviderDto) {
    const updated = await jarvisApi.updateProvider(provider, dto)
    setProviders(prev => prev.map(p => p.provider === provider ? { ...p, ...updated } : p))
  }

  if (loading) return <div className="text-text-tertiary text-sm p-4">Chargement…</div>
  if (error)   return (
    <div className="flex items-center gap-2 text-danger text-sm p-4">
      <XCircle size={16} /> {error}
    </div>
  )

  return (
    <div>
      <div className="flex items-center gap-2 mb-6">
        <Settings2 size={20} className="text-primary" />
        <h2 className="text-lg font-semibold text-text-primary">Fournisseurs LLM</h2>
      </div>
      <p className="text-sm text-text-secondary mb-5">
        Configurez les fournisseurs de modèles de langage. Les modifications nécessitent un redémarrage du service Jarvis pour les clés API.
      </p>
      <div className="grid gap-4">
        {providers.map(p => (
          <ProviderCard key={p.provider} config={p} onUpdate={handleUpdate} />
        ))}
      </div>
    </div>
  )
}
