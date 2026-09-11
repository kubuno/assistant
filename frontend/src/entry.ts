import { RouteRegistry, WaffleAppRegistry, ModuleSettingsRegistry, useSidebarStore, useToolbarStore, useRightPanelStore, SDK_VERSION, FaviconRegistry } from '@kubuno/sdk'
/** Bundle MODULE assistant — chargé à l'exécution (cf. vite.module.config). */
import { lazy } from 'react'
import AssistantLogo from './AssistantLogo'
import AssistantMiniPanel from './AssistantMiniPanel'
import './index.css'
import './i18n'
import AssistantSidebarBody from './components/AssistantSidebarBody'
import { registerAssistantAdmin } from './admin/AssistantAdminPanel'

export const sdkVersion = SDK_VERSION

export function register() {
  // Assistant has its own logo: the tab shows it under /assistant.
  FaviconRegistry.register('assistant', '/assistant-logo.png')

  WaffleAppRegistry.register('assistant', 'Assistant', [
    { id: 'assistant', label: 'Assistant', Icon: AssistantLogo, path: '/assistant' },
  ])

  // The header gear button opens the per-user Assistant settings while in /assistant.
  ModuleSettingsRegistry.register('assistant')

  // Instance administration (core console ▸ Modules ▸ Assistant): provider
  // credentials, which a generated form cannot express.
  registerAssistantAdmin()

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

  // Side panel: resume a conversation from anywhere.
  useRightPanelStore.getState().registerEntry({
    moduleId:       'assistant',
    icon:           AssistantLogo,
    label:          'Assistant',
    panelComponent: AssistantMiniPanel,
    openPath:       '/assistant',
  })

  // Routes
  const AssistantPage         = lazy(() => import('./AssistantPage'))
  const AssistantSettingsPage = lazy(() => import('./AssistantSettingsPage'))

  RouteRegistry.register('assistant',           AssistantPage)
  RouteRegistry.register('assistant/settings',  AssistantSettingsPage)
  RouteRegistry.register('assistant/:convId',   AssistantPage)
}
