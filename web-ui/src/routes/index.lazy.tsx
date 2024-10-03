import { createLazyFileRoute } from '@tanstack/react-router'
import HomePage from '../pages/home/home'

export const Route = createLazyFileRoute('/')({
  component: HomePage
})