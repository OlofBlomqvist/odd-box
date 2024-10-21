import { createFileRoute } from '@tanstack/react-router'
import SitePage from '../pages/site/site'

type SiteSearchParams = {
  hostname: string,
  tab?: number
}

export const Route = createFileRoute('/site')({
  component: SitePage,
  validateSearch: (search:SiteSearchParams) => {
    return {
      hostname: search.hostname,
      tab: search.tab ?? 0,
    }
  }
})

