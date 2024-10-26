import { useState } from "react";
import { useRouter } from "@tanstack/react-router";
import { Popover, PopoverContent, PopoverTrigger } from "../popover/popover";

export const DirectoryServersMenuActions = () => {
  const [isPopoverOpen, setIsPopoverOpen] = useState(false);
  const router = useRouter();

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
              router.navigate({ to: "/new-dirserver" });
            }}
            className="button-dropdown-option"
            style={{ width: "100%", height: "36px" }}
          >
            New directory server
          </button>
        </div>
      </PopoverContent>
    </Popover>
  );
};
