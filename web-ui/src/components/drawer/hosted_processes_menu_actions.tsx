import { useState } from "react";
import toast from "react-hot-toast";
import { useRouter } from "@tanstack/react-router";
import useSiteMutations from "../../hooks/use-site-mutations";
import { Popover, PopoverContent, PopoverTrigger } from "../popover/popover";
export const HostedProcessesMenuActions = () => {
  const [isPopoverOpen, setIsPopoverOpen] = useState(false);
  const router = useRouter();
  const { startSite, stopSite } = useSiteMutations();

  return (
    <Popover open={isPopoverOpen}>
      <PopoverTrigger asChild>
        <div
          className="w-7 h-7 p-0 opacity-50 rounded-[5px] grid place-content-center cursor-pointer transition-all duration-100 border border-transparent hover:border-white/50"
          onClick={() => setIsPopoverOpen(true)}
        >
          •••
        </div>
      </PopoverTrigger>
      <PopoverContent
        onInteractOutside={() => setIsPopoverOpen(false)}
        onClick={() => setIsPopoverOpen(false)}
        className="max-w-[max-content] border bg-white text-black"
      >
        <div
          style={{
            background: "white",
            marginTop: "1px",
            borderRadius: "4px",
          }}
        >
          <button
            onClick={() => {
              router.navigate({ to: "/new-process" });
            }}
            className="button-dropdown-option"
            style={{ width: "100%", height: "36px" }}
          >
            New hosted process
          </button>

          <button
            onClick={() => {
              toast.promise(startSite.mutateAsync({ hostname: "*" }), {
                loading: "Starting all sites..",
                success: "All site started!",
                error: (e) => `Failed to start sites. ${e}`,
              });
            }}
            className="button-dropdown-option"
            style={{
              width: "100%",
              borderTop: "1px solid var(--color4)",
              borderBottom: "1px solid var(--color4)",
              height: "36px",
            }}
          >
            Start all sites
          </button>

          <button
            onClick={() => {
              toast.promise(stopSite.mutateAsync({ hostname: "*" }), {
                loading: "Stopping all sites..",
                success: "All site stopped!",
                error: (e) => `Failed to stop sites. ${e}`,
              });
            }}
            className="button-dropdown-option"
            style={{
              width: "100%",
              height: "36px",
            }}
          >
            Stop all sites
          </button>
        </div>
      </PopoverContent>
    </Popover>
  );
};
