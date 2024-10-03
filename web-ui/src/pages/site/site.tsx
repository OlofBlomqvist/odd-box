import { useParams } from "@tanstack/react-router";
import SiteOverview from "./site-overview";
import SiteSettings from "./site-settings";
import SiteLogs from "./site-logs";
import Tabs from "../../components/tabs/tabs";
import useHostedSites from "../../hooks/use-hosted-sites";
import { useRemoteSites } from "../../hooks/use-remote-sites";

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
      content: (
        <SiteOverview
          hostedProcess={thisHostedProcess}
          remoteSite={thisRemoteSite}
        />
      ),
    });
  }

  tabSections.push({
    name: "Settings",
    content: (
      <SiteSettings
        hostedProcess={thisHostedProcess}
        remoteSite={thisRemoteSite}
      />
    ),
  });

  if (thisHostedProcess) {
    tabSections.push({
      name: "Log",
      content: (
        <SiteLogs
          hostedProcess={thisHostedProcess}
          remoteSite={thisRemoteSite}
        />
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
