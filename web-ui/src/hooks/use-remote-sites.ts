import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";
import checkIsRemoteSite from "../lib/is_remote_site";

export const useRemoteSites = () => {
  const apiClient = new Api({ baseUrl: import.meta.env.VITE_ODDBOX_API_URL });

  return useSuspenseQuery({
    queryKey: ["sites"],
    queryFn: apiClient.sites.list,
    select: (res) =>
      res.data.items.filter(checkIsRemoteSite).map((x) => x.RemoteSite),
  });
};
