import toast from "react-hot-toast";
import useSiteStatus from "../../hooks/use-site-status";
import useSiteMutations from "../../hooks/use-site-mutations";
import { BasicProcState, InProcessSiteConfig } from "../../generated-api";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/cn";
import useSettings from "@/hooks/use-settings";

const SiteOverview = ({
  hostedProcess,
}: {
  hostedProcess: InProcessSiteConfig;
}) => {
  const {data:settings} = useSettings()
  const { startSite, stopSite } = useSiteMutations();
  const siteStatus = useSiteStatus();
  const thisSiteStatus =
    siteStatus.data?.find((x) => x.hostname === hostedProcess.host_name)
      ?.state ?? BasicProcState.Stopped;

  return (
    <main
      style={{ display: "flex", width: "100%" }}
      className="sm:gap-6 gap-4 flex max-w-[900px] flex-col sm:flex-row"
    >
      <div className="flex-grow flex flex-col gap-4 sm:gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Process details</CardTitle>
            <CardDescription>
              General information for{" "}
              <span className="font-bold text-[var(--accent-text)]">
                {hostedProcess.host_name}
              </span>
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col gap-4">
              <div>
                <p className="text-sm mb-1">Hostname</p>
                <Input disabled value={hostedProcess.host_name} />
              </div>
              {hostedProcess && (
                <div>
                  <p className="text-sm mb-1">Port</p>
                  <Input disabled value={hostedProcess.port ?? settings.http_port} />
                </div>
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Metrics</CardTitle>
            <CardDescription>
              Metrics is not yet available for{" "}
              <span className="font-bold text-[var(--accent-text)]">
                {hostedProcess.host_name}
              </span>
            </CardDescription>
          </CardHeader>
        </Card>
      </div>

      <div className="flex flex-col gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Status</CardTitle>
            <CardDescription>
              Current status for{" "}
              <span className="font-bold text-[var(--accent-text)]">
                {hostedProcess?.host_name}
              </span>
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Badge
              className={cn(
                thisSiteStatus === BasicProcState.Running &&
                  "bg-green-800 text-white"
              )}
            >
              {thisSiteStatus}
            </Badge>
          </CardContent>
        </Card>

        <Card className="flex-grow-[.25] h-[max-content]">
          <CardHeader>
            <CardTitle>Actions</CardTitle>
            <CardDescription>
              Available actions for{" "}
              <span className="font-bold text-[var(--accent-text)]">
                {hostedProcess?.host_name}
              </span>
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col gap-2">
              <Button loadingText="Starting.." isLoading={startSite.isPending}
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
                    startSite.mutateAsync({
                      hostname: hostedProcess.host_name,
                    }),
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
              <Button loadingText="Stopping.." isLoading={stopSite.isPending}
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
          </CardContent>
        </Card>
      </div>
    </main>
  );
};

export default SiteOverview;
