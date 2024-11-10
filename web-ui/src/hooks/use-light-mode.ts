import { useQuery } from "@tanstack/react-query";

const useLightMode = () => {
  return useQuery({
    queryKey: ["light-mode"],
    queryFn: async () => {
      return window.localStorage.getItem("light-mode") === "true";
    },
  });
};

export default useLightMode