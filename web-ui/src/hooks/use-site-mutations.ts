import { useMutation, useQueryClient } from "@tanstack/react-query";
import Sleep from "../lib/sleep";
import { useRouter } from "@tanstack/react-router";
import {
  Api,
  BasicProcState,
  InProcessSiteConfig,
  RemoteSiteConfig,
} from "../generated-api";

const useSiteMutations = () => {
  let hostName = window.location.protocol + "//" + window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl = import.meta.env.MODE === "development" ? import.meta.env.VITE_ODDBOX_API_URL : hostName;
  
  const apiClient = new Api({ baseUrl });
  const router = useRouter();
  const queryClient = useQueryClient();

  const startSite = useMutation({
    mutationKey: ["start-site"],
    mutationFn: async ({ hostname }: { hostname: string }) => {
      await apiClient.sites.start({ hostname });

      const isAllSitesRequest = hostname === "*";

      await Sleep(isAllSitesRequest ? 3000 : 1000);

      if (!isAllSitesRequest) {
        let newStates = (await apiClient.sites.status()).data;
        let thisSiteState = newStates.items.find(
          (x: any) => x.hostname === hostname
        )?.state;
        let maxRetries = 5;
        let retryAttempt = 0;
        while (
          retryAttempt < maxRetries &&
          thisSiteState !== BasicProcState.Running
        ) {
          retryAttempt++;
          await Sleep(1000);
          newStates = (await apiClient.sites.status()).data;

          thisSiteState = newStates.items.find(
            (x: any) => x.hostname === hostname
          )?.state;
        }

        queryClient.invalidateQueries({ queryKey: ["site-status"] });
        if (thisSiteState !== BasicProcState.Running) {
          throw new Error("Site did not start");
        }
      } else {
        queryClient.invalidateQueries({ queryKey: ["site-status"] });
      }
    },
  });

  const stopSite = useMutation({
    mutationKey: ["stop-site"],
    mutationFn: async ({ hostname }: { hostname: string }) => {
      await apiClient.sites.stop({ hostname });

      const isAllSitesRequest = hostname === "*";

      await Sleep(isAllSitesRequest ? 3000 : 1000);

      if (!isAllSitesRequest) {
        let newStates = (await apiClient.sites.status()).data;
        let thisSiteState = newStates.items.find(
          (x: any) => x.hostname === hostname
        )?.state;
        let maxRetries = 5;
        let retryAttempt = 0;

        while (
          retryAttempt < maxRetries &&
          thisSiteState !== BasicProcState.Stopped
        ) {
          retryAttempt++;
          await Sleep(1000);
          newStates = (await apiClient.sites.status()).data;

          thisSiteState = newStates.items.find(
            (x: any) => x.hostname === hostname
          )?.state;
        }
        queryClient.invalidateQueries({ queryKey: ["site-status"] });

        if (thisSiteState !== BasicProcState.Stopped) {
          throw new Error("Site did not stop");
        }
      } else {
        queryClient.invalidateQueries({ queryKey: ["site-status"] });
      }
    },
  });

  const updateRemoteSite = useMutation({
    mutationKey: ["update-remote-site"],
    mutationFn: ({
      hostname,
      siteSettings,
    }: {
      siteSettings: RemoteSiteConfig;
      hostname?: string;
    }) => {
      return apiClient.sites.set(
        {
          new_configuration: {
            RemoteSite: siteSettings,
          },
        },
        {
          hostname,
        }
      );
    },
    onSettled: (_x, _y, vars) => {
      queryClient.invalidateQueries({ queryKey: ["sites"] });
      if (vars.hostname !== vars.siteSettings.host_name) {
        router.navigate({
          to: `/site/${vars.siteSettings.host_name.replace("http://", "").replace("https://", "")}`,
          search: { tab: 1 },
        });
      }
    },
  });

  const updateSite = useMutation({
    mutationKey: ["update-site"],
    mutationFn: ({
      hostname,
      siteSettings,
    }: {
      siteSettings: InProcessSiteConfig;
      hostname?: string;
    }) => {
      return apiClient.sites.set(
        {
          new_configuration: {
            HostedProcess: siteSettings,
          },
        },
        {
          hostname,
        }
      );
    },
    onSettled: (_x, _y, vars) => {
      queryClient.invalidateQueries({ queryKey: ["sites"] });
      if (vars.hostname !== vars.siteSettings.host_name) {
        router.navigate({
          to: `/site/${vars.siteSettings.host_name.replace("http://", "").replace("https://", "")}`,
          search: { tab: 1 },
        });
      }
    },
  });

  const deleteSite = useMutation({
    mutationKey: ["delete-site"],
    mutationFn: async ({ hostname }: { hostname: string }) => {
      await apiClient.sites.delete({ hostname });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sites"] });
    },
  });

  return {
    startSite,
    stopSite,
    updateSite,
    deleteSite,
    updateRemoteSite,
  };
};

export default useSiteMutations;
