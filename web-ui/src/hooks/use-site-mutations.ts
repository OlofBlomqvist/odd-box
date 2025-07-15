import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";
import {
  Api,
  DirServer,
  InProcessSiteConfig,
  RemoteSiteConfig,
} from "../generated-api";
import { getUrlFriendlyUrl } from "@/lib/get_url_friendly_url";
import { getCookie } from "@/utils/cookies";

const useSiteMutations = () => {
  let hostName = window.location.protocol + "//" + window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl =
    import.meta.env.MODE === "development"
      ? `${import.meta.env.VITE_ODDBOX_API_URL}:${import.meta.env.VITE_ODDBOX_API_PORT}`
      : hostName;
  
  
  const apiClient = new Api({ baseUrl, securityWorker: () => {
    const password = getCookie("password");  
      return {
        headers: [["Authorization", `${password || ""}`]]
      };
  } });
  
  const router = useRouter();
  const queryClient = useQueryClient();

  const startSite = useMutation({
    mutationKey: ["start-site"],
    mutationFn: apiClient.api.start,
  });

  const stopSite = useMutation({
    mutationKey: ["stop-site"],
    mutationFn: apiClient.api.stop,
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
      return apiClient.api.set(
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
          to: `/site`,
          search: { hostname: getUrlFriendlyUrl(vars.siteSettings.host_name), tab: 0 },
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
      return apiClient.api.set(
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
          to: `/site`,
          search: { tab: 1, hostname: getUrlFriendlyUrl(vars.siteSettings.host_name) },
        });
      }
    },
  });

  const updateDirServer = useMutation({
    mutationKey: ["update--dir-server"],
    mutationFn: ({
      hostname,
      siteSettings,
    }: {
      siteSettings: DirServer;
      hostname?: string;
    }) => {
      return apiClient.api.set(
        {
          new_configuration: {
            DirServer: siteSettings,
          },
        },
        {
          hostname,
        }
      );
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["sites"] });
    },
  });

  const deleteSite = useMutation({
    mutationKey: ["delete-site"],
    mutationFn: apiClient.api.delete,
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
    updateDirServer,
  };
};

export default useSiteMutations;
