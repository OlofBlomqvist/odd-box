import { useParams } from "@tanstack/react-router";
import SiteOverview from "./site-overview";
import SiteLogs from "./site-logs";
import Tabs from "../../components/tabs/tabs";
import useHostedSites from "../../hooks/use-hosted-sites";
import { useRemoteSites } from "../../hooks/use-remote-sites";
import { Card, CardContent } from "@/components/ui/card";
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

  if (!thisHostedProcess && !thisRemoteSite) {
    return <p>site not found</p>;
  }

  const tabSections = [];

  if (thisHostedProcess) {
    tabSections.push({
      name: "Overview",
      content: <SiteOverview hostedProcess={thisHostedProcess} />,
    });
  }

  if (thisRemoteSite) {
    tabSections.push({
      name: "Settings",
      content: <RemoteSiteSettings site={thisRemoteSite} />,
    });
  } else if (thisHostedProcess) {
    tabSections.push({
      name: "Settings",
      content: <HostedProcessSettings site={thisHostedProcess} />,
    });
  }

  if (thisHostedProcess) {
    tabSections.push({
      name: "Log",
      content: (
        <main className="grid flex-1 items-start gap-4 sm:py-0 md:gap-8 max-w-[900px]">
          <Card>
            <CardContent>
              <SiteLogs
                hostedProcess={thisHostedProcess}
                remoteSite={thisRemoteSite}
              />
            </CardContent>
          </Card>
        </main>
      ),
    });
  }

  return (
    <div>
      <p
        style={{
          textTransform: "uppercase",
          fontSize: ".9rem",
          fontWeight: "bold",
          color: "var(--color2)",
        }}
        className="pl-[20px] md:pl-0"
      >
        {params.siteName}
      </p>
      <Tabs
        key={thisHostedProcess?.host_name ?? thisRemoteSite?.host_name}
        sections={tabSections}
      />
    </div>
  );
};

export default SitePage;
