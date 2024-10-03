import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Api } from "../generated-api";

const useSettingsMutations = () => {
  const apiClient = new Api({ baseUrl: import.meta.env.VITE_ODDBOX_API_URL });
  const queryClient = useQueryClient();
  const updateSettings = useMutation({
    mutationKey: ["update-settings"],
    mutationFn: apiClient.settings.saveSettings,
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });

  return {
    updateSettings,
  };
};

export default useSettingsMutations;
