import { createFileRoute } from '@tanstack/react-router'
import { LoginPage } from '../pages/login/login'
import { z } from 'zod'

export const Route = createFileRoute('/login')({
  component: LoginPage,
  validateSearch: z.object({
    redirect: z.string().optional().default('/')
  })
})