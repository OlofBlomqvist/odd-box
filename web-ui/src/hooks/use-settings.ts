import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";

const useSettings = () => {
  let hostName = window.location.protocol + "//" + window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl =
    import.meta.env.MODE === "development"
      ? `${import.meta.env.VITE_ODDBOX_API_URL}:${import.meta.env.VITE_ODDBOX_API_PORT}`
      : hostName;
  
  const apiClient = new Api({ baseUrl });

  return useSuspenseQuery({
    queryKey: ["settings"],
    select: (response) => response.data,
    queryFn: apiClient.api.settings,
  });
};

export default useSettings;
