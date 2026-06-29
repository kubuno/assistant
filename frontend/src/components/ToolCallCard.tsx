import { useState } from 'react'
import { Wrench, Check, AlertTriangle, Play, X, Loader2, MousePointerClick } from 'lucide-react'
import { jarvisApi, JarvisToolCall } from '../api'

// Human-readable one-line summary of the tool arguments.
function argsSummary(args: Record<string, unknown>): string {
  return Object.entries(args)
    .map(([k, v]) => `${k}: ${typeof v === 'object' ? JSON.stringify(v) : String(v)}`)
    .join('  ·  ')
}

/** Renders one assistant tool call: a backend result, a dispatched UI action,
 *  or a `confirm`-gated action awaiting the user's explicit approval. */
export default function ToolCallCard({ call }: { call: JarvisToolCall }) {
  const [phase, setPhase]   = useState<'idle' | 'running' | 'done' | 'cancelled'>('idle')
  const [result, setResult] = useState<{ text: string; error: boolean } | null>(null)

  const run = async () => {
    setPhase('running')
    try {
      const r = await jarvisApi.callTool(call.tool, call.args)
      setResult({ text: r.result, error: r.is_error })
    } catch (e) {
      setResult({ text: e instanceof Error ? e.message : String(e), error: true })
    }
    setPhase('done')
  }

  const summary = argsSummary(call.args)

  // ── UI action (already dispatched client-side) ──────────────────────────────
  if (call.kind === 'ui') {
    return (
      <div className="flex items-center gap-2 rounded-xl border border-border bg-surface-1 px-3 py-2 text-xs">
        <MousePointerClick size={14} className="text-primary flex-shrink-0" />
        <span className="font-medium text-text-primary">{call.tool}</span>
        {summary && <span className="text-text-tertiary truncate">{summary}</span>}
        <span className="ml-auto text-text-tertiary">action exécutée</span>
      </div>
    )
  }

  // ── Confirmation-gated action ───────────────────────────────────────────────
  if (call.kind === 'confirm') {
    return (
      <div className="rounded-xl border border-warning/40 bg-warning/5 px-3 py-2.5 text-xs">
        <div className="flex items-center gap-2">
          <AlertTriangle size={14} className="text-warning flex-shrink-0" />
          <span className="font-medium text-text-primary">{call.tool}</span>
          <span className="text-text-tertiary">confirmation requise</span>
        </div>
        {summary && <div className="mt-1 ml-6 text-text-secondary break-words">{summary}</div>}

        {phase === 'idle' && (
          <div className="mt-2 ml-6 flex items-center gap-2">
            <button
              onClick={run}
              className="flex items-center gap-1 rounded-full bg-danger px-3 py-1 font-medium text-white hover:bg-danger/90 transition-colors"
            >
              <Play size={11} /> Exécuter
            </button>
            <button
              onClick={() => setPhase('cancelled')}
              className="flex items-center gap-1 rounded-full bg-surface-2 px-3 py-1 font-medium text-text-secondary hover:bg-surface-3 transition-colors"
            >
              <X size={11} /> Annuler
            </button>
          </div>
        )}
        {phase === 'running' && (
          <div className="mt-2 ml-6 flex items-center gap-1.5 text-text-secondary">
            <Loader2 size={12} className="animate-spin" /> Exécution…
          </div>
        )}
        {phase === 'cancelled' && (
          <div className="mt-2 ml-6 text-text-tertiary">Action annulée.</div>
        )}
        {phase === 'done' && result && (
          <ResultBlock text={result.text} error={result.error} />
        )}
      </div>
    )
  }

  // ── Backend tool already executed by the agentic loop ───────────────────────
  const isError = !!call.is_error
  return (
    <div className="rounded-xl border border-border bg-surface-1 px-3 py-2.5 text-xs">
      <div className="flex items-center gap-2">
        <Wrench size={14} className="text-text-secondary flex-shrink-0" />
        <span className="font-medium text-text-primary">{call.tool}</span>
        {isError
          ? <AlertTriangle size={13} className="text-danger" />
          : <Check size={13} className="text-success" />}
      </div>
      {summary && <div className="mt-1 ml-6 text-text-secondary break-words">{summary}</div>}
      {call.result && <ResultBlock text={call.result} error={isError} />}
    </div>
  )
}

function ResultBlock({ text, error }: { text: string; error: boolean }) {
  // Pretty-print JSON results when possible.
  let pretty = text
  try { pretty = JSON.stringify(JSON.parse(text), null, 2) } catch { /* keep raw */ }
  return (
    <pre className={`mt-2 ml-6 max-h-40 overflow-auto rounded-lg px-2 py-1.5 text-[11px] leading-relaxed whitespace-pre-wrap break-words ${
      error ? 'bg-danger/5 text-danger' : 'bg-surface-2 text-text-secondary'
    }`}>{pretty}</pre>
  )
}
