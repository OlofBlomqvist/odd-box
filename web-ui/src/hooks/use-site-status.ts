import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";

const useSiteStatus = () => {
  let hostName = window.location.protocol + "//" + window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl = import.meta.env.MODE === "development" ? import.meta.env.VITE_ODDBOX_API_URL : hostName;
  
  const apiClient = new Api({ baseUrl });
  return useSuspenseQuery({
    queryKey: ["site-status"],
    queryFn: apiClient.api.status,
    select: (response) => {
        return response.data.items
    }
  });
};

export default useSiteStatus
