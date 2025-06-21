import { createFileRoute } from '@tanstack/react-router'
import HomePage from '../pages/home/home'

type HomeSearchParams = {
  type?: string
}

export const Route = createFileRoute('/')({
  component: HomePage,
  validateSearch: (search: HomeSearchParams) => {
    // Only return the search params without modifying them
    // This prevents automatic URL updates that can cause loops
    return {
      type: search.type
    }
  },
  loader: ({ location }) => { 
    // Access search params through the location object
    const searchParams = new URLSearchParams(location.search);
    const type = searchParams.get('type') || "processes";
    return { defaultType: type }
  }
})