import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";

const useSettings = () => {
  const apiClient = new Api({ baseUrl: import.meta.env.VITE_ODDBOX_API_URL });

  return useSuspenseQuery({
    queryKey: ["settings"],
    select: (response) => response.data,
    queryFn: apiClient.settings.settings,
  });
};

export default useSettings;
