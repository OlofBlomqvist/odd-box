import MenuItem from "./menu_item";
import StatusIcon from "./status-icon";
import useSiteStatus from "../../hooks/use-site-status";
import useHostedSites from "../../hooks/use-hosted-sites";
import { PlusIcon } from "lucide-react";
import { getUrlFriendlyUrl } from "@/lib/get_url_friendly_url";

const HostedProcessesList = () => {
  const { data: hostedSites } = useHostedSites();
  const { data: siteStatus } = useSiteStatus();

  const hostedSitesOrdered = hostedSites.sort((a, b) =>
    a.host_name.localeCompare(b.host_name)
  );

  return (
    <>
      {hostedSitesOrdered.map((site) => {
        const siteState = siteStatus.find(
          (x: any) => x.hostname === site.host_name
        )?.state;
        return (
          <MenuItem
            key={site.host_name}
            rightIcon={
              <StatusIcon state={siteState} hostname={site.host_name} />
            }
            title={site.host_name}
            to={`/site`}
            searchParams={{ hostname: getUrlFriendlyUrl(site.host_name) }}
            icon={null}
          />
        );
      })}

      {hostedSites.length === 0 && (
        <MenuItem
          fontSize=".9rem"
          title="Add new"
          to="/new-process"
          icon={<PlusIcon />}
        />
      )}
    </>
  );
};

export default HostedProcessesList;
