import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";

const useSiteStatus = () => {
  const apiClient = new Api({ baseUrl: import.meta.env.VITE_ODDBOX_API_URL });
  return useSuspenseQuery({
    queryKey: ["site-status"],
    queryFn: apiClient.sites.status,
    select: (response) => {
        return response.data.items
    }
  });
};

export default useSiteStatus
