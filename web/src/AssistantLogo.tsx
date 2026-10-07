interface AssistantLogoProps {
  size?:      number
  className?: string
  title?:     string
}

/** Assistant logo (designer artwork, raster). Served by the host from
 *  `/assistant-logo.png`; rendered as a square image so it weighs the same as
 *  its neighbours in the waffle menu. */
export function AssistantLogo({ size = 24, className, title = 'Assistant' }: AssistantLogoProps) {
  return (
    <img
      src="/assistant-logo.png"
      width={size}
      height={size}
      alt={title}
      className={className}
      style={{ display: 'block', objectFit: 'contain' }}
    />
  )
}

export default AssistantLogo
