import { createFileRoute } from '@tanstack/react-router'
import HomePage from '../pages/home/home'

type HomeSearchParams = {
  type: string
}

export const Route = createFileRoute('/')({
  component: HomePage,
  validateSearch: (search:HomeSearchParams) => {  
    return {
      type: search.type ?? "processes",
    }
  }
})
