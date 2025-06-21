import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";
import checkIsRemoteSite from "../lib/is_remote_site";
import { deleteCookie, getCookie } from "@/utils/cookies"; 

export const useRemoteSites = () => {
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
    queryFn: async () => { 
          try {
              const password = getCookie("password");  
              var result = await apiClient.api.list({
                headers: {
                  Authorization: `${password || ""}`
                } 
              }); 
              return result;
            } catch(e:any) {
              if(e.status === 403) {
                deleteCookie("password");
              }
              throw e;
            }},
    select: (res) =>
      res.data.items.filter(checkIsRemoteSite).map((x) => x.RemoteSite),
  });
};
