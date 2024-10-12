import { createLazyFileRoute } from '@tanstack/react-router'
import SitePage from '../pages/site/site'

export const Route = createLazyFileRoute('/site/$siteName')({
  component: SitePage,
})

