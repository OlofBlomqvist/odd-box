import MenuItem from "./menu_item";
import StatusIcon from "./status-icon";
import useSiteStatus from "../../hooks/use-site-status";
import { useRemoteSites } from "../../hooks/use-remote-sites";
import { PlusIcon } from "lucide-react";
import { getUrlFriendlyUrl } from "@/lib/get_url_friendly_url";

const RemoteSitesList = () => {
  const { data: remoteSites } = useRemoteSites();
  const { data: siteStatus } = useSiteStatus();

  const remoteSitesOrdered = remoteSites.sort((a, b) =>
    a.host_name.localeCompare(b.host_name)
  );

  return (
    <>
      {remoteSitesOrdered.map((site) => {
        const siteState = siteStatus.find(
          (x: any) => x.hostname === site.host_name
        )?.state;
        return (
          <MenuItem
            key={site.host_name}
            rightIcon={
              <StatusIcon
                state={siteState}
                hostname={site.host_name}
                isRemoteSite
              />
            }
            title={site.host_name}
            to={`/site`}
            searchParams={{ hostname: getUrlFriendlyUrl(site.host_name) }}
            icon={null}
          />
        );
      })}
      {remoteSitesOrdered.length === 0 && (
        <MenuItem
          fontSize=".9rem"
          title="Add new"
          to="/new-site"
          icon={<PlusIcon />}
        />
      )}
    </>
  );
};

export default RemoteSitesList;
