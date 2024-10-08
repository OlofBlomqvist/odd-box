import NewProcessPage from '@/pages/new-process/new-process'
import { createLazyFileRoute } from '@tanstack/react-router'

export const Route = createLazyFileRoute('/new-process')({
  component: NewProcessPage
})