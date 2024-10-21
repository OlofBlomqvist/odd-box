import { createFileRoute } from '@tanstack/react-router'
import LogsPage from '../pages/logs/logs'

type LogsSearchParams = {
  hostname: string
}

export const Route = createFileRoute('/logs')({
  component: LogsPage,
  validateSearch: (search:LogsSearchParams) => {
    return {
      hostname: search.hostname,
    }
  }
})
