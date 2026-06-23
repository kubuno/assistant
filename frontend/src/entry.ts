/** Bundle MODULE jarvis — chargé à l'exécution (cf. vite.module.config). */
import { lazy } from 'react'
import { RouteRegistry, WaffleAppRegistry, ModuleSettingsRegistry, useSidebarStore, useToolbarStore, SDK_VERSION } from '@kubuno/sdk'
import { Bot } from 'lucide-react'
import './index.css'
import './i18n'
import JarvisSidebarBody from './components/JarvisSidebarBody'

export const sdkVersion = SDK_VERSION

export function register() {
  WaffleAppRegistry.register('jarvis', 'Jarvis', [
    { id: 'jarvis', label: 'Jarvis', Icon: Bot, path: '/jarvis' },
  ])

  // The header gear button opens the per-user Jarvis settings while in /jarvis.
  ModuleSettingsRegistry.register('jarvis')

  useToolbarStore.getState().register({
    moduleId:    'jarvis',
    routePrefix: '/jarvis',
    noPadding:   true,
  })

  useSidebarStore.getState().register({
    moduleId:    'jarvis',
    routePrefix: '/jarvis',
    SidebarBody: JarvisSidebarBody,
    collapsedBody: true,
  })

  // Routes
  const JarvisPage         = lazy(() => import('./JarvisPage'))
  const JarvisSettingsPage = lazy(() => import('./JarvisSettingsPage'))

  RouteRegistry.register('jarvis',           JarvisPage)
  RouteRegistry.register('jarvis/settings',  JarvisSettingsPage)
  RouteRegistry.register('jarvis/:convId',   JarvisPage)
}
