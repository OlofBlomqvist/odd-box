import { ConfigurationItem, InProcessSiteConfig } from "../generated-api";

export const checkIsHostedProcess = (
  site: ConfigurationItem
): site is {
  HostedProcess: InProcessSiteConfig;
} => {
  return "HostedProcess" in site;
};

export default checkIsHostedProcess;
