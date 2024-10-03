import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";
import checkIsHostedProcess from "../lib/is_hosted_process";

export const useHostedSites = () => {
  const apiClient = new Api({ baseUrl: import.meta.env.VITE_ODDBOX_API_URL });

  return useSuspenseQuery({
    queryKey: ["sites"],
    queryFn: apiClient.sites.list,
    select: (res) =>
      res.data.items.filter(checkIsHostedProcess).map((x) => x.HostedProcess),
  });
};
export default useHostedSites;
