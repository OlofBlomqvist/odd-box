import { createLazyFileRoute } from '@tanstack/react-router'
import SettingsPage from '../pages/settings/settings'

export const Route = createLazyFileRoute('/settings')({
  component: SettingsPage
})