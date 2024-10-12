import { createLazyFileRoute } from '@tanstack/react-router'
import NewSitePage from '../pages/new-site/new-site'

export const Route = createLazyFileRoute('/new-site')({
  component: NewSitePage
})