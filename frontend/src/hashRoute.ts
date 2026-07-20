/**
 * Addressable sub-views that have no dedicated router route.
 *
 * The module page lives at `/jarvis`; the selected conversation is a state of
 * that page, not a route. To keep every clickable element in the sidebar a real
 * link (and to make direct links / the browser Back button work), such views are
 * encoded in the URL hash: `/jarvis/#<kind>/<id>`.
 *
 * `hashTo` builds the link target (usable as `<Link to={…}>`), `fromHash` parses
 * `useLocation().hash` back into the view it designates.
 */

export type HashKind = 'conversation'

/** Link target for an addressable sub-view, e.g. `/jarvis/#conversation/<id>`. */
export function hashTo(kind: HashKind, id: string): string {
  return `/jarvis/#${kind}/${encodeURIComponent(id)}`
}

/** Parse a `useLocation().hash` value back into `{ kind, id }`, or null. */
export function fromHash(hash: string): { kind: HashKind; id: string } | null {
  const raw = hash.startsWith('#') ? hash.slice(1) : hash
  const m = /^([a-z]+)\/(.+)$/.exec(raw)
  if (!m) return null
  const kind = m[1]
  if (kind !== 'conversation') return null
  return { kind, id: decodeURIComponent(m[2]) }
}
