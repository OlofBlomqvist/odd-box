import "./style.css";
import Button from "../../components/button/button";
import toast from "react-hot-toast";
import useSiteStatus from "../../hooks/use-site-status";
import useSiteMutations from "../../hooks/use-site-mutations";
import {
  BasicProcState,
  InProcessSiteConfig,
  RemoteSiteConfig,
} from "../../generated-api";

const stateToButtonText = {
  [BasicProcState.Running]: "Stop site",
  [BasicProcState.Stopped]: "Start site",
  [BasicProcState.Faulty]: "Start site",
  [BasicProcState.Stopping]: "Start site",
  [BasicProcState.Starting]: "Stop site",
  [BasicProcState.Remote]: "Remote",
};

const SiteOverview = ({
  hostedProcess,
  remoteSite,
}: {
  hostedProcess?: InProcessSiteConfig;
  remoteSite?: RemoteSiteConfig;
}) => {
  const { startSite, stopSite } = useSiteMutations();
  const siteStatus = useSiteStatus();
  const thisSiteStatus = hostedProcess
    ? siteStatus.data?.find((x) => x.hostname === hostedProcess.host_name)
        ?.state
    : remoteSite
      ? siteStatus.data?.find((x) => x.hostname === remoteSite.host_name)?.state
      : BasicProcState.Remote;

  return (
    <>
      <div
        style={{
          fontSize: ".8rem",
          display: "flex",
          flexDirection: "column",
          gap: "10px",
          maxWidth: "750px",
        }}
      >
        <div
          className="pb-2 select-none flex items-center justify-between border-b border-gray-500"
          title={`${thisSiteStatus}`}
        >
          <p style={{ textTransform: "uppercase", fontWeight: "bold" }}>
            status:
          </p>
          <p style={{ display: "flex", alignItems: "center", gap: "5px" }}>
            {thisSiteStatus}
          </p>
        </div>

        <div style={{ marginTop: "10px" }}>
          {thisSiteStatus !== BasicProcState.Remote && (
            <Button
              style={{ maxWidth: "max-content" }}
              disabled={startSite.isPending || stopSite.isPending}
              dangerButton={thisSiteStatus === BasicProcState.Running}
              onClick={async () => {
                if (!hostedProcess) {
                  return;
                }

                if (thisSiteStatus === BasicProcState.Running) {
                  toast.promise(
                    stopSite.mutateAsync({ hostname: hostedProcess.host_name }),
                    {
                      loading: "Stopping site..",
                      success: "Site stopped!",
                      error: (e) => `Failed to stop site: ${e}`,
                    }
                  );
                } else {
                  toast.promise(
                    startSite.mutateAsync({
                      hostname: hostedProcess.host_name,
                    }),
                    {
                      loading: "Starting site..",
                      success: "Site started!",
                      error: (e) => `Failed to start site: ${e}`,
                    }
                  );
                }
              }}
            >
              {stateToButtonText[thisSiteStatus!]}
            </Button>
          )}
        </div>
      </div>
    </>
  );
};

export default SiteOverview;
