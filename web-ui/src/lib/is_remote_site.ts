import { ConfigurationItem, RemoteSiteConfig } from "../generated-api";

export const checkIsRemoteSite = (
  site: ConfigurationItem
): site is {
  RemoteSite: RemoteSiteConfig;
} => {
  return "RemoteSite" in site;
};

export default checkIsRemoteSite;
