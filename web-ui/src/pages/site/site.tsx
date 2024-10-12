import { useParams } from "@tanstack/react-router";
import SiteOverview from "./site-overview";
import SiteLogs from "./site-logs";
import Tabs from "../../components/tabs/tabs";
import useHostedSites from "../../hooks/use-hosted-sites";
import { useRemoteSites } from "../../hooks/use-remote-sites";
import RemoteSiteSettings from "./remote-site-settings";
import HostedProcessSettings from "./hosted-process-settings";

const SitePage = () => {
  const { data: sites } = useHostedSites();
  const { data: remoteSites } = useRemoteSites();
  const params = useParams({ from: "/site/$siteName" });

  const thisHostedProcess = sites.find(
    (x) =>
      x.host_name.replaceAll("http://", "").replaceAll("https://", "") ===
      params.siteName
  );
  const thisRemoteSite = remoteSites.find(
    (x) =>
      x.host_name.replaceAll("http://", "").replaceAll("https://", "") ===
      params.siteName
  );

  if (thisRemoteSite) {
    return (
      <RemoteSiteSettings
        key={thisRemoteSite.host_name}
        site={thisRemoteSite}
      />
    );
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
          <SiteLogs hostedProcess={thisHostedProcess} />
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
