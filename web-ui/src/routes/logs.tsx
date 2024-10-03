import { createFileRoute } from '@tanstack/react-router'
import LogsPage from '../pages/logs/logs'

export const Route = createFileRoute('/logs')({
  component: LogsPage
})