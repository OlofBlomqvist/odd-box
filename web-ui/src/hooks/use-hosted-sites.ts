import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";
import checkIsHostedProcess from "../lib/is_hosted_process";

export const useHostedSites = () => {
  let hostName = window.location.protocol + "//" + window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl = import.meta.env.MODE === "development" ? import.meta.env.VITE_ODDBOX_API_URL : hostName;
  
  const apiClient = new Api({ baseUrl });

  return useSuspenseQuery({
    queryKey: ["sites"],
    queryFn: apiClient.sites.list,
    select: (res) =>
      res.data.items.filter(checkIsHostedProcess).map((x) => x.HostedProcess),
  });
};
export default useHostedSites;
