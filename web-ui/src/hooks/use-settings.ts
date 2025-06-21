import { useSuspenseQuery } from "@tanstack/react-query";
import { Api } from "../generated-api";
import { deleteCookie, getCookie } from "@/utils/cookies";

const useSettings = () => {
  let hostName = window.location.protocol + "//" + window.location.hostname
  if (window.location.port) {
    hostName = `${hostName}:${window.location.port}`
  }

  const baseUrl =
    import.meta.env.MODE === "development"
      ? `${import.meta.env.VITE_ODDBOX_API_URL}:${import.meta.env.VITE_ODDBOX_API_PORT}`
      : hostName;
  
  
  const apiClient = new Api({ baseUrl, });

  return useSuspenseQuery({
    queryKey: ["settings"],
    select: (response) => response.data,
   queryFn: async () => { 
         try {
             const password = getCookie("password");  
             var result = await apiClient.api.settings({
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
  });
};

export default useSettings;
