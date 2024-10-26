import MenuItem from "./menu_item";
import { PlusIcon } from "lucide-react";
import { getUrlFriendlyUrl } from "@/lib/get_url_friendly_url";
import { useDirServers } from "@/hooks/use-dir-servers";

const DirServersList = () => {
  const { data: dirServers } = useDirServers();

  const orderedDirServers = dirServers.sort((a, b) =>
    a.host_name.localeCompare(b.host_name)
  );

  return (
    <>
      {orderedDirServers.map((site) => {
        return (
          <MenuItem
            key={site.host_name}
            title={site.host_name}
            to={`/site`}
            searchParams={{ hostname: getUrlFriendlyUrl(site.host_name) }}
            icon={null}
          />
        );
      })}
      {orderedDirServers.length === 0 && (
        <MenuItem
          fontSize=".9rem"
          title="Add new"
          to="/new-dirserver"
          icon={<PlusIcon />}
        />
      )}
    </>
  );
};

export default DirServersList;
