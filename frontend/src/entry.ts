/** Bundle MODULE assistant — chargé à l'exécution (cf. vite.module.config). */
import { lazy } from 'react'
import { RouteRegistry, WaffleAppRegistry, ModuleSettingsRegistry, useSidebarStore, useToolbarStore, SDK_VERSION } from '@kubuno/sdk'
import { Bot } from 'lucide-react'
import './index.css'
import './i18n'
import AssistantSidebarBody from './components/AssistantSidebarBody'

export const sdkVersion = SDK_VERSION

export function register() {
  WaffleAppRegistry.register('assistant', 'Assistant', [
    { id: 'assistant', label: 'Assistant', Icon: Bot, path: '/assistant' },
  ])

  // The header gear button opens the per-user Assistant settings while in /assistant.
  ModuleSettingsRegistry.register('assistant')

  useToolbarStore.getState().register({
    moduleId:    'assistant',
    routePrefix: '/assistant',
    noPadding:   true,
  })

  useSidebarStore.getState().register({
    moduleId:    'assistant',
    routePrefix: '/assistant',
    SidebarBody: AssistantSidebarBody,
    collapsedBody: true,
  })

  // Routes
  const AssistantPage         = lazy(() => import('./AssistantPage'))
  const AssistantSettingsPage = lazy(() => import('./AssistantSettingsPage'))

  RouteRegistry.register('assistant',           AssistantPage)
  RouteRegistry.register('assistant/settings',  AssistantSettingsPage)
  RouteRegistry.register('assistant/:convId',   AssistantPage)
}
