import "./style.css";
// import Button from "../../components/button/button";
import toast from "react-hot-toast";
import useSiteStatus from "../../hooks/use-site-status";
import useSiteMutations from "../../hooks/use-site-mutations";
import {
  BasicProcState,
  InProcessSiteConfig,
  RemoteSiteConfig,
} from "../../generated-api";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

const stateToButtonText = {
  [BasicProcState.Running]: "Stop",
  [BasicProcState.Stopped]: "Start",
  [BasicProcState.Faulty]: "Start",
  [BasicProcState.Stopping]: "Start",
  [BasicProcState.Starting]: "Stop",
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
      <div
        style={{ display: "flex", width: "100%" }}
        className="sm:gap-6 gap-4 flex max-w-[750px] flex-col sm:flex-row"
      >
        <div className="flex-grow flex flex-col gap-4 sm:gap-6">
          <Card className="p-4 border2">
            <h1 className="text-base font-bold mb-3 uppercase">Site details</h1>
            {/* <p className="text-base">View and manage this site</p> */}

            <div className="flex flex-col gap-4">
              <div>
                <p className="text-sm mb-1">Hostname</p>
                <Input
                  disabled
                  value={hostedProcess?.host_name ?? remoteSite?.host_name}
                />
              </div>
              {hostedProcess && (
                <div>
                  <p className="text-sm mb-1">Port</p>
                  <Input disabled value={hostedProcess.port!} />
                </div>
              )}
            </div>
          </Card>


          {/* TODO: THIS IS SOME DESIGN FOR METRICS, BUT WE DONT HAVE THIS THROUGH THE API YET. */}
          {/* <Card className="p-5">
            <h1 className="text-base font-bold mb-3 uppercase">Metrics</h1>

            <div className="grid gap-4">
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <div className="text-sm font-medium text-muted-foreground">
                    Requests
                  </div>
                  <div className="text-4xl font-bold">12,345</div>
                </div>
                <div>
                  <div className="text-sm font-medium text-muted-foreground">
                    Errors
                  </div>
                  <div className="text-4xl font-bold">23</div>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div>
                  <div className="text-sm font-medium text-muted-foreground">
                    Bandwidth
                  </div>
                  <div className="text-4xl font-bold">2,3 GB</div>
                </div>
                <div>
                  <div className="text-sm font-medium text-muted-foreground">
                    Response Time
                  </div>
                  <div className="text-4xl font-bold">125 ms</div>
                </div>
              </div>
            </div>
          </Card> */}
        </div> 


        <Card className="p-4 flex-grow-[.25] h-[max-content]">
          <h1 className="text-base font-bold uppercase">Actions</h1>
          <div className="mt-2 flex flex-col gap-2">
            <Button
              disabled={
                startSite.isPending ||
                stopSite.isPending ||
                thisSiteStatus === BasicProcState.Running
              }
              onClick={() => {
                if (!hostedProcess) {
                  return;
                }
                toast.promise(
                  startSite.mutateAsync({ hostname: hostedProcess.host_name }),
                  {
                    loading: "Starting site..",
                    success: "Site started!",
                    error: (e) => `Failed to start site: ${e}`,
                  }
                );
              }}
              size={"sm"}
              className="w-full uppercase font-bold"
            >
              start
            </Button>
            <Button
              disabled={
                startSite.isPending ||
                stopSite.isPending ||
                thisSiteStatus === BasicProcState.Stopped
              }
              onClick={() => {
                if (!hostedProcess) {
                  return;
                }
                toast.promise(
                  stopSite.mutateAsync({ hostname: hostedProcess.host_name }),
                  {
                    loading: "Stopping site..",
                    success: "Site stopped!",
                    error: (e) => `Failed to stop site: ${e}`,
                  }
                );
              }}
              size={"sm"}
              className="w-full uppercase font-bold"
            >
              stop
            </Button>
          </div>
        </Card>
      </div>
  );
};

export default SiteOverview;
