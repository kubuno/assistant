import { useRef, useEffect, useState, KeyboardEvent } from 'react'
import { Plus, Square, Mic, ImagePlus, X, ChevronDown } from 'lucide-react'
import { useVoiceDictation } from '@kubuno/sdk'
import { MenuDropdown, useMenuDropdown, type MenuItem } from '@ui'
import { useJarvisStore } from '../jarvisStore'

// ── Main component ────────────────────────────────────────────────────────────

interface Props {
  convId?:       string
  onConvCreated?: (id: string) => void
  inputId?:      string
}

export default function ChatInput({ convId, onConvCreated, inputId }: Props) {
  const [value, setValue]           = useState('')
  const textareaRef                 = useRef<HTMLTextAreaElement>(null)
  const fileInputRef                = useRef<HTMLInputElement>(null)
  // Menus déroulants via le composant primaire @ui (portail + clamp viewport :
  // plus de div flottant maison rognée par l'overflow de la carte).
  const plusMenu  = useMenuDropdown()
  const modelMenu = useMenuDropdown()
  // Pièces jointes images (data URLs pour aperçu + envoi futur).
  const [images, setImages]         = useState<{ id: string; name: string; url: string }[]>([])

  const {
    isStreaming, selectedModel, models,
    addUserMessage, streamChat, stopStream,
    createConversation, setSelectedModel, setSelectedProvider,
  } = useJarvisStore()

  const plusItems: MenuItem[] = [
    { type: 'action', label: 'Ajouter des images', icon: <ImagePlus size={16} />,
      onClick: () => fileInputRef.current?.click() },
  ]
  const modelItems: MenuItem[] = models.length
    ? models.map(m => ({
        type: 'action' as const,
        label: m.is_default ? `${m.name} · par défaut` : m.name,
        shortcut: m.provider,
        checked: m.id === selectedModel,
        onClick: () => { setSelectedModel(m.id); setSelectedProvider(m.provider) },
      }))
    : [{ type: 'label' as const, text: 'Aucun modèle configuré' }]

  // Ajout d'images : lecture en data URL pour l'aperçu (et l'envoi à venir).
  const addImageFiles = (files: FileList | null) => {
    if (!files) return
    Array.from(files).filter(f => f.type.startsWith('image/')).forEach(f => {
      const reader = new FileReader()
      reader.onload = () => setImages(prev => [
        ...prev,
        { id: `${f.name}-${f.size}-${prev.length}-${f.lastModified}`, name: f.name, url: String(reader.result) },
      ])
      reader.readAsDataURL(f)
    })
  }
  const removeImage = (id: string) => setImages(prev => prev.filter(i => i.id !== id))

  // Auto-resize textarea
  useEffect(() => {
    const ta = textareaRef.current
    if (!ta) return
    ta.style.height = 'auto'
    ta.style.height = Math.min(ta.scrollHeight, 200) + 'px'
  }, [value])

  // ── Saisie vocale (hook partagé du SDK : même toast éditable centré que la
  //    barre de recherche du core, branché sur le backend STT auto-hébergé). La
  //    dictée part du texte déjà saisi et le met à jour en direct. ──
  // Validating the dictation (Enter / ✓ in the toast) sends the message directly.
  const voice = useVoiceDictation({ getSeed: () => value, onText: setValue, onSubmit: (t) => submit(t) })

  const stop = () => stopStream()

  const submit = async (override?: string) => {
    const content = (override ?? value).trim()
    if (!content || isStreaming) return
    setValue('')
    setImages([])  // pièces jointes consommées

    // If no convId, create one first
    let targetConvId = convId
    if (!targetConvId) {
      targetConvId = await createConversation({ title: content.slice(0, 60) })
      onConvCreated?.(targetConvId)
    }

    addUserMessage(content)
    await streamChat(targetConvId, { content })
  }

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submit() }
  }

  const currentModel = models.find(m => m.id === selectedModel)
  const modelLabel   = currentModel?.name ?? 'Modèle'

  return (
    <div className="px-4 py-4 no-print">
      <div className="max-w-3xl mx-auto">
        {/* Input card */}
        <div
          className="flex flex-col bg-white border border-border rounded-3xl shadow-sm
                     focus-within:border-primary/50 focus-within:shadow-md transition-all overflow-hidden"
        >
          {/* Aperçu des images jointes */}
          {images.length > 0 && (
            <div className="flex flex-wrap gap-2 px-4 pt-3">
              {images.map(img => (
                <div key={img.id} className="relative group/img">
                  <img src={img.url} alt={img.name}
                    className="w-16 h-16 object-cover rounded-xl border border-border" />
                  <button
                    onClick={() => removeImage(img.id)}
                    title="Retirer"
                    className="absolute -top-1.5 -right-1.5 w-5 h-5 rounded-full bg-text-primary/80 text-white
                               flex items-center justify-center opacity-0 group-hover/img:opacity-100 transition-opacity"
                  >
                    <X size={12} />
                  </button>
                </div>
              ))}
            </div>
          )}

          {/* Textarea row */}
          <div className="flex items-center gap-2 px-4 pt-3 pb-1">
            <input
              ref={fileInputRef}
              type="file"
              accept="image/*"
              multiple
              hidden
              onChange={e => { addImageFiles(e.target.files); e.target.value = '' }}
            />
            <button
              onClick={plusMenu.open}
              title="Ajouter du contenu"
              className={`flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center transition-colors
                         ${plusMenu.isOpen ? 'bg-surface-2 text-text-primary' : 'text-text-secondary hover:bg-surface-2'}`}
            >
              <Plus size={18} className={`transition-transform ${plusMenu.isOpen ? 'rotate-45' : ''}`} />
            </button>
            <textarea
              id={inputId}
              ref={textareaRef}
              value={value}
              onChange={e => setValue(e.target.value)}
              onKeyDown={onKeyDown}
              placeholder="Demander à Jarvis"
              rows={1}
              disabled={isStreaming}
              className="flex-1 resize-none bg-transparent text-sm text-text-primary
                         placeholder:text-text-tertiary focus:outline-none
                         min-h-[28px] max-h-[200px] leading-7 disabled:opacity-60 py-0"
            />
          </div>

          {/* Bottom row: model selector + send */}
          <div className="flex items-center justify-between px-4 pb-3 pt-1">
            <div className="flex items-center gap-1">
              {/* Model selector */}
              <button
                onClick={modelMenu.open}
                className="flex items-center gap-1 text-xs font-medium text-text-secondary
                           hover:text-text-primary hover:bg-surface-2 rounded-full px-3 py-1.5 transition-colors"
              >
                <span className="w-1.5 h-1.5 rounded-full bg-primary flex-shrink-0" />
                {modelLabel}
                <ChevronDown size={12} className="ml-0.5" />
              </button>
            </div>

            <div className="flex items-center gap-1.5">
              {/* Saisie vocale — toast éditable centré (hook partagé du SDK).
                  Masquée si l'admin a désactivé la reconnaissance ou si le
                  module STT est absent. */}
              {voice.enabled && (
                <button
                  onClick={voice.toggleVoice}
                  disabled={isStreaming}
                  title={voice.listening ? 'Arrêter la dictée' : 'Saisie vocale'}
                  className={`w-9 h-9 rounded-full flex items-center justify-center transition-colors disabled:opacity-40 ${
                    (voice.listening || voice.voiceLoading) ? 'bg-danger/15 text-danger animate-pulse' : 'text-text-secondary hover:bg-surface-2'
                  }`}
                >
                  <Mic size={17} />
                </button>
              )}

              {/* Send / Stop */}
              <button
                onClick={isStreaming ? stop : () => submit()}
                disabled={!isStreaming && !value.trim()}
                className="w-9 h-9 rounded-full flex items-center justify-center transition-all
                           disabled:opacity-40 disabled:cursor-not-allowed
                           bg-primary hover:bg-primary-hover text-white"
              >
                {isStreaming
                  ? <Square size={14} fill="currentColor" />
                  : <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"/></svg>
                }
              </button>
            </div>
          </div>
        </div>

        <p className="text-xs text-text-tertiary text-center mt-2">
          Jarvis peut faire des erreurs. Vérifiez les informations importantes.
        </p>
      </div>

      {/* Menus déroulants (composant primaire @ui, rendus en portail) */}
      {plusMenu.isOpen  && plusMenu.pos  && <MenuDropdown items={plusItems}  pos={plusMenu.pos}  onClose={plusMenu.close} />}
      {modelMenu.isOpen && modelMenu.pos && <MenuDropdown items={modelItems} pos={modelMenu.pos} onClose={modelMenu.close} minWidth={240} />}

      {/* Toast vocal centré (partagé) */}
      {voice.voiceToast}
    </div>
  )
}
