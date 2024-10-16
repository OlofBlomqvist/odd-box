import MenuItem from "./menu_item";
import StatusIcon from "./status-icon";
import useSiteStatus from "../../hooks/use-site-status";
import useHostedSites from "../../hooks/use-hosted-sites";
import { useRemoteSites } from "../../hooks/use-remote-sites";
import { PlusIcon } from "lucide-react";

const SitesList = () => {
  const { data: hostedSites } = useHostedSites();
  const { data: remoteSites } = useRemoteSites();
  const { data: siteStatus } = useSiteStatus();

  const hostedSitesOrdered = hostedSites.sort((a, b) =>
    a.host_name.localeCompare(b.host_name)
  );
  const remoteSitesOrdered = remoteSites.sort((a, b) =>
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
            href={`/site/${site.host_name.replaceAll("http://", "").replaceAll("https://", "")}`}
            icon={null}
          />
        );
      })}
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
            href={`/site/${site.host_name.replaceAll("http://", "").replaceAll("https://", "")}`}
            icon={null}
          />
        );
      })}
      {hostedSites.length === 0 && remoteSites.length === 0 && (
        <MenuItem
          fontSize=".9rem"
          title="NEW SITE"
          href="/new-site"
          icon={<PlusIcon />}
        />
      )}
    </>
  );
};

export default SitesList;
