import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";
import checkIsRemoteSite from "../lib/is_remote_site";

export const useRemoteSites = () => {
  let hostName = window.location.protocol + "//" + window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl = import.meta.env.MODE === "development" ? import.meta.env.VITE_ODDBOX_API_URL : hostName;
  
  const apiClient = new Api({ baseUrl });

  return useSuspenseQuery({
    queryKey: ["sites"],
    queryFn: apiClient.api.list,
    select: (res) =>
      res.data.items.filter(checkIsRemoteSite).map((x) => x.RemoteSite),
  });
};
