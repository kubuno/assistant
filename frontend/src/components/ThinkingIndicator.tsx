export default function ThinkingIndicator() {
  return (
    <div className="flex items-center gap-2 px-4 py-3 text-text-secondary">
      <div className="flex gap-1">
        {[0, 1, 2].map(i => (
          <span
            key={i}
            className="w-2 h-2 rounded-full bg-primary animate-bounce"
            style={{ animationDelay: `${i * 0.15}s` }}
          />
        ))}
      </div>
      <span className="text-sm">Jarvis réfléchit…</span>
    </div>
  )
}
