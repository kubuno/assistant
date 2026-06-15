import { useAuthStore } from '@kubuno/sdk'

interface Props {
  content: string
}

export default function UserMessage({ content }: Props) {
  const user = useAuthStore(s => s.user)
  const initials = (user?.display_name ?? user?.username ?? 'U')
    .split(' ')
    .map(w => w[0])
    .join('')
    .slice(0, 2)
    .toUpperCase()

  return (
    <div className="flex gap-3 justify-end">
      <div className="max-w-[75%]">
        <div className="bg-primary text-white rounded-2xl rounded-tr-md px-4 py-3 text-sm leading-relaxed whitespace-pre-wrap">
          {content}
        </div>
      </div>
      <div className="flex-shrink-0 w-8 h-8 rounded-full bg-primary text-white text-xs font-medium flex items-center justify-center">
        {initials}
      </div>
    </div>
  )
}
