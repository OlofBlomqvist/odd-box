import { createLazyFileRoute } from '@tanstack/react-router'
import NewDirServerPage from '../pages/new-dirserver/new-dirserver'

export const Route = createLazyFileRoute('/new-dirserver')({
  component: NewDirServerPage
})