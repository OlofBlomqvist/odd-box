import SiteOverview from "./site-overview";
import SiteLogs from "./site-logs";
import Tabs from "../../components/tabs/tabs";
import useHostedSites from "../../hooks/use-hosted-sites";
import { useRemoteSites } from "../../hooks/use-remote-sites";
import RemoteSiteSettings from "./remote-site-settings";
import HostedProcessSettings from "./hosted-process-settings";
import { Route } from "@/routes/site";
import { getUrlFriendlyUrl } from "@/lib/get_url_friendly_url";
import { useDirServers } from "@/hooks/use-dir-servers";
import DirServerSettings from "./dir-server-settings";

const SitePage = () => {
  const { hostname } = Route.useSearch();
  
  const { data: sites } = useHostedSites();
  const { data: remoteSites } = useRemoteSites();
  const { data: dirServers } = useDirServers();
  
  const thisHostedProcess = sites.find(
    (x) =>
      getUrlFriendlyUrl(x.host_name) ===
      hostname
  );
  const thisRemoteSite = remoteSites.find(
    (x) =>
      getUrlFriendlyUrl(x.host_name) ===
      hostname
  );
  const thisDirServer = dirServers.find(
    (x) =>
      getUrlFriendlyUrl(x.host_name) ===
      hostname
  );

  if (thisRemoteSite) {
    return (
      <RemoteSiteSettings
        key={thisRemoteSite.host_name}
        site={thisRemoteSite}
      />
    );
  }

  if (thisDirServer) {
    return <DirServerSettings site={thisDirServer} />;
  }

  if (!thisHostedProcess) {
    return <p>hosted process not found..</p>;
  }

  const tabSections = [
    {
      name: "Overview",
      content: <SiteOverview hostedProcess={thisHostedProcess} />,
    },
    {
      name: "Settings",
      content: <HostedProcessSettings site={thisHostedProcess} />,
    },
    {
      name: "Logs",
      content: (
        <main className="grid flex-1 items-start gap-4 sm:py-0 md:gap-8 max-w-[900px]">
          <SiteLogs host={hostname} />
        </main>
      ),
    },
  ];

  return (
    <div>
      <Tabs key={thisHostedProcess?.host_name} sections={tabSections} />
    </div>
  );
};

export default SitePage;
