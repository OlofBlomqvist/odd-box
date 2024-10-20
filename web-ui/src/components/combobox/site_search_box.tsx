import * as React from "react";
import { CaretSortIcon } from "@radix-ui/react-icons";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/command/command";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/popover/popover";
import useHostedSites from "@/hooks/use-hosted-sites";
import { useRemoteSites } from "@/hooks/use-remote-sites";
import useSiteStatus from "@/hooks/use-site-status";
import { useRouter } from "@tanstack/react-router";
import { getUrlFriendlyUrl } from "@/lib/get_url_friendly_url";

export function SiteSearchBox() {
  const { data: hostedSites } = useHostedSites();
  const { data: remoteSites } = useRemoteSites();
  const { data: siteStatus } = useSiteStatus();
  const [open, setOpen] = React.useState(false);
  const router = useRouter();
  const options = [...hostedSites, ...remoteSites].map((site) => ({
    value: site.host_name,
    label: site.host_name,
    status: siteStatus.find((x: any) => x.hostname === site.host_name)?.state,
  }));

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger>
        <div
          role="combobox"
          aria-expanded={open}
          className="w-[200px] justify-between flex items-center rounded pl-2 pr-2 h-[32px] border-[rgba(255,255,255,0.2)] border"
        >
          Find site...
          <CaretSortIcon className="ml-2 h-4 w-4 shrink-0 opacity-50" />
        </div>
      </PopoverTrigger>
      <PopoverContent className="w-[200px] p-0 border-[rgba(255,255,255,0.2)] bg-[rgba(36,36,36,1)]">
        <Command className="bg-[rgba(36,36,36,1)]">
          <CommandInput placeholder="Search sites..." className="h-9" />
          <CommandList>
            <CommandEmpty className="text-white p-3 text-sm">
              No site found
            </CommandEmpty>
            <CommandGroup>
              {options.map((framework) => (
                <CommandItem
                  className="hover:bg-[rgba(255,255,255,0.1)] hover:cursor-pointer text-white"
                  key={framework.value}
                  value={framework.value}
                  onSelect={(currentValue: any) => {
                    router.navigate({
                      to: `/site`,
                      search: {
                        hostname: getUrlFriendlyUrl(currentValue),
                        tab: 0
                      }
                    });
                    setOpen(false);
                  }}
                >
                  {framework.label}
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
