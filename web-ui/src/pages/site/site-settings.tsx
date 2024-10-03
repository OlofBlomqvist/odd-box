import "./style.css";
import HostedProcessSettings from "./hosted-process-settings";
import RemoteSiteSettings from "./remote-site-settings";
import { InProcessSiteConfig, RemoteSiteConfig } from "../../generated-api";

const SiteSettings = ({
  hostedProcess,
  remoteSite,
}: {
  hostedProcess?: InProcessSiteConfig;
  remoteSite?: RemoteSiteConfig;
}) => {
  if (hostedProcess) {
    return <HostedProcessSettings site={hostedProcess} />;
  } else if (remoteSite) {
    return <RemoteSiteSettings site={remoteSite} />;
  }

  return <p>Site not found</p>;
};

export default SiteSettings;
