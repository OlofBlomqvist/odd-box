import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";
import checkIsDirServer from "@/lib/is_dir_server";

export const useDirServers = () => {
  let hostName = window.location.protocol + "//" + window.location.hostname;
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`;
  }

  const baseUrl =
    import.meta.env.MODE === "development"
      ? `${import.meta.env.VITE_ODDBOX_API_URL}:${import.meta.env.VITE_ODDBOX_API_PORT}`
      : hostName;

  const apiClient = new Api({ baseUrl });

  return useSuspenseQuery({
    queryKey: ["sites"],
    queryFn: apiClient.api.list,
    select: (res) =>
      res.data.items.filter(checkIsDirServer).map((x) => x.DirServer),
  });
};