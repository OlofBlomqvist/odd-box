import MenuItem from "./menu_item";
import { useRemoteSites } from "../../hooks/use-remote-sites";
import { PlusIcon } from "lucide-react";
import { getUrlFriendlyUrl } from "@/lib/get_url_friendly_url";

const RemoteSitesList = () => {
  const { data: remoteSites } = useRemoteSites();

  const remoteSitesOrdered = remoteSites.sort((a, b) =>
    a.host_name.localeCompare(b.host_name)
  );

  return (
    <>
      {remoteSitesOrdered.map((site) => {
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
      {remoteSitesOrdered.length === 0 && (
        <MenuItem
          fontSize=".9rem"
          title="Add new"
          to="/new-site"
          icon={<PlusIcon />}
        />
      )}
    </>
  );
};

export default RemoteSitesList;
