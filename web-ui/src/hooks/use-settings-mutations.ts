import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Api } from "../generated-api";

const useSettingsMutations = () => {
  let hostName = window.location.protocol + "//" + window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl =
    import.meta.env.MODE === "development"
      ? `${import.meta.env.VITE_ODDBOX_API_URL}:${import.meta.env.VITE_ODDBOX_API_PORT}`
      : hostName;
  
  const apiClient = new Api({ baseUrl });
  const queryClient = useQueryClient();
  const updateSettings = useMutation({
    mutationKey: ["update-settings"],
    mutationFn: apiClient.api.saveSettings,
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });

  return {
    updateSettings,
  };
};

export default useSettingsMutations;
