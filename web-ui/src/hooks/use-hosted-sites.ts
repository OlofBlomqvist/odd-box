import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";
import checkIsHostedProcess from "../lib/is_hosted_process";
import { deleteCookie, getCookie } from "@/utils/cookies"; 

export const useHostedSites = () => {
  let hostName = window.location.protocol + "//" + window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl =
    import.meta.env.MODE === "development"
      ? `${import.meta.env.VITE_ODDBOX_API_URL}:${import.meta.env.VITE_ODDBOX_API_PORT}`
      : hostName;
  
  
  const apiClient = new Api({ baseUrl});

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
      res.data.items.filter(checkIsHostedProcess).map((x) => x.HostedProcess),
  });
};
export default useHostedSites;
