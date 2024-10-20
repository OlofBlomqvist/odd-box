import { useState } from "react";
import toast from "react-hot-toast";
import useSiteMutations from "../../hooks/use-site-mutations";
import { BasicProcState } from "../../generated-api";
import { Popover, PopoverContent } from "../popover/popover";
import { PopoverTrigger } from "@radix-ui/react-popover";
import { cx } from "class-variance-authority";

const statusColors = {
  Running: "greenyellow",
  Stopped: "var(--color1)",
  Disabled: "gray",
  Stopping: "yellow",
  Starting: "yellow",
  Remote: "white",
  Faulty: "yellow",
};

const StatusIcon = ({
  hostname,
  state,
  isRemoteSite,
}: {
  isRemoteSite?: boolean;
  state?: BasicProcState;
  hostname: string;
}) => {
  const [isPopoverOpen, setIsPopoverOpen] = useState(false);
  const { startSite, stopSite } = useSiteMutations();

  return (
    <Popover open={isPopoverOpen}>
      <PopoverTrigger asChild>
        <span
          className="w-7 h-7 p-0 rounded-[5px] grid place-content-center cursor-pointer transition-all duration-100 border border-transparent hover:border-white/50"
          title={state}
          onClick={(e) => {
            e.preventDefault();
            if (isRemoteSite) {
              return;
            }
            setIsPopoverOpen(true);
          }}
          style={{
            color: statusColors[state!],
            fontSize: isRemoteSite ? ".7rem" : "1rem",
          }}
        >
          {isRemoteSite ? "R" : "●"}
        </span>
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
            overflow: "hidden",
          }}
        >
          <button
            disabled={state === BasicProcState.Running}
            onClick={() => {
              toast.promise(startSite.mutateAsync({ hostname }), {
                loading: `Starting site.. [${hostname}]`,
                success: `Site started! [${hostname}]`,
                error: (e) => `Failed to start site: ${e}`,
              });
            }}
            className={cx("button-dropdown-option", state === BasicProcState.Running && "opacity-50")}
            style={{
              width: "100%",
              borderBottom: "1px solid var(--color4)",
              height: "36px",
            }}
          >
            Start site
          </button>

          <button
            disabled={state !== BasicProcState.Running}
            onClick={() => {
              toast.promise(stopSite.mutateAsync({ hostname }), {
                loading: `Stopping site.. [${hostname}]`,
                success: `Site stopped! [${hostname}]`,
                error: (e) => `Failed to stop site: ${e}`,
              });
            }}
            className={cx("button-dropdown-option",state !== BasicProcState.Running && "opacity-50")}
            style={{
              width: "100%",
              height: "36px",
              borderBottom: "1px solid var(--color4)",
            }}
          >
            Stop site
          </button>
        </div>
      </PopoverContent>
    </Popover>
  );
};

export default StatusIcon;
