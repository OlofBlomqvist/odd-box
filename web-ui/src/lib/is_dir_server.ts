import { ConfigurationItem, DirServer } from "../generated-api";

export const checkIsDirServer = (
  DirServer: ConfigurationItem
): DirServer is {
  DirServer: DirServer;
} => {
  return "DirServer" in DirServer;
};

export default checkIsDirServer;
