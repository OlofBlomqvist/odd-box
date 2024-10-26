import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Tabs, TabsContent, TabsList } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import {
  ActivityIcon,
  Ellipsis,
  GlobeIcon,
  PlusSquareIcon,
  Server,
} from "lucide-react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import useHostedSites from "@/hooks/use-hosted-sites";
import { useRemoteSites } from "@/hooks/use-remote-sites";
import { useRouter } from "@tanstack/react-router";
import useSiteStatus from "@/hooks/use-site-status";
import { cn } from "@/lib/cn";
import { BasicProcState } from "@/generated-api";
import { SlidingTabBar } from "@/components/ui/sliding_tabs/sliding_tabs";
import { getUrlFriendlyUrl } from "@/lib/get_url_friendly_url";
import InfoCard from "./info-card";
import { useDirServers } from "@/hooks/use-dir-servers";
const HomePage = () => {
  const { data: hostedProcesses } = useHostedSites();
  const { data: remoteSites } = useRemoteSites();
  const { data: siteStatus } = useSiteStatus();
  const { data: dirServers } = useDirServers();
  const router = useRouter();
  const searchParams = new URLSearchParams(window.location.search);
  const type = searchParams.get("type");

  const slidingTabs = [
    { label: "Processes", value: "processes" },
    { label: "Sites", value: "sites" },
    { label: "Servers", value: "servers" },
  ]

  return (
    <main className="grid flex-1 items-start gap-4 sm:py-0 md:gap-8 max-w-[900px]">
      <div className="grid auto-rows-max items-start gap-4 md:gap-8">
        <Card x-chunk="dashboard-06-chunk-0">
          <CardHeader>
            <CardTitle>Dashboard</CardTitle>
            <CardDescription>
              Get an overview of your proxy configuration and status.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-4">
              <InfoCard
                title="Processes"
                icon={<ActivityIcon className="h-4 w-4" />}
                leftData={{ label: "Total", value: hostedProcesses.length }}
                rightData={{
                  label: "Running",
                  value: siteStatus.filter(
                    (site) => site.state === BasicProcState.Running
                  ).length,
                }}
              />
              <InfoCard
                title="Sites"
                icon={<GlobeIcon className="h-4 w-4" />}
                leftData={{ label: "Total", value: remoteSites.length }}
              />
              <InfoCard
                title="Directory Servers"
                icon={<Server className="h-4 w-4" />}
                leftData={{ label: "Total", value: dirServers.length }}
              />
            </div>
          </CardContent>
        </Card>

        <Tabs
          defaultValue={slidingTabs.find(x => x.value === type)?.value ?? "processes"}
          className="pb-8"
        >
          <div className="flex items-center">
            <TabsList>
              <SlidingTabBar defaultTabIndex={slidingTabs.findIndex(x => x.value === type) ?? 0}
                tabs={slidingTabs}
              />
            </TabsList>
          </div>

          <TabsContent value="processes">
            <Card x-chunk="dashboard-06-chunk-1">
              <CardHeader className="flex items-center justify-between flex-row">
                <div>
                <CardTitle>Processes</CardTitle>
                <CardDescription>
                  Viewing all hosted processes and their status.
                </CardDescription>
                </div>
                <Button
                onClick={() => {
                  router.navigate({
                    to: "/new-process",
                  });
                }}
                className="opacity-[.9] flex gap-2 border border-transparent hover:border-white/20"
                variant="ghost"
              >
                <PlusSquareIcon className="min-w-4" /> New process
              </Button>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Site</TableHead>
                      <TableHead className="text-right pr-6">Status</TableHead>
                      <TableHead className="text-right hidden sm:table-cell">
                        Actions
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {hostedProcesses.map((hostedProcess) => {
                      const state = siteStatus.find(
                        (x) => x.hostname === hostedProcess.host_name
                      )?.state;
                      return (
                        <TableRow
                          key={hostedProcess.host_name}
                          className="hover:cursor-pointer"
                          onClick={() => {
                            router.navigate({
                              to: `/site`,
                              search: {
                                tab: 1,
                                hostname: getUrlFriendlyUrl(
                                  hostedProcess.host_name
                                ),
                              },
                            });
                          }}
                        >
                          <TableCell>
                            <div className="font-bold  overflow-hidden text-ellipsis max-w-[30ch]">
                              {hostedProcess.host_name}
                            </div>
                            <div className="text-xs text-muted-foreground overflow-hidden text-ellipsis max-w-[30ch]">
                              {hostedProcess.bin}
                            </div>
                          </TableCell>
                          <TableCell className="text-right">
                            <Badge
                              variant="secondary"
                              className={cn(
                                state === BasicProcState.Running &&
                                  "bg-green-800"
                              )}
                            >
                              {state}
                            </Badge>
                          </TableCell>
                          <TableCell className="text-right hidden sm:table-cell">
                            <DropdownMenu>
                              <DropdownMenuTrigger asChild>
                                <Button
                                  aria-haspopup="true"
                                  size="icon"
                                  variant="ghost"
                                >
                                  <Ellipsis className="h-4 w-4" />
                                  <span className="sr-only">Toggle menu</span>
                                </Button>
                              </DropdownMenuTrigger>
                              <DropdownMenuContent align="end">
                                <DropdownMenuLabel>Actions</DropdownMenuLabel>
                                <DropdownMenuItem
                                  onClick={() => {
                                    router.navigate({
                                      to: `/site`,
                                      search: {
                                        tab: 0,
                                        hostname: getUrlFriendlyUrl(
                                          hostedProcess.host_name
                                        ),
                                      },
                                    });
                                  }}
                                >
                                  View
                                </DropdownMenuItem>
                                <DropdownMenuItem
                                  onClick={() => {
                                    router.navigate({
                                      to: `/site`,
                                      search: {
                                        tab: 1,
                                        hostname: getUrlFriendlyUrl(
                                          hostedProcess.host_name
                                        ),
                                      },
                                    });
                                  }}
                                >
                                  Edit
                                </DropdownMenuItem>
                                <DropdownMenuItem
                                  onClick={() => {
                                    router.navigate({
                                      to: `/site`,
                                      search: {
                                        tab: 2,
                                        hostname: getUrlFriendlyUrl(
                                          hostedProcess.host_name
                                        ),
                                      },
                                    });
                                  }}
                                >
                                  Logs
                                </DropdownMenuItem>
                              </DropdownMenuContent>
                            </DropdownMenu>
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="sites">
            <Card x-chunk="dashboard-06-chunk-1">
              <CardHeader className="flex items-center justify-between flex-row">
                <div>
                  <CardTitle>Sites</CardTitle>
                <CardDescription>Viewing all remote sites</CardDescription>
                </div>
                <Button
                onClick={() => {
                  router.navigate({
                    to: "/new-site",
                  });
                }}
                className="opacity-[.9] flex gap-2 border border-transparent hover:border-white/20 m-0"
                variant="ghost"
              >
                <PlusSquareIcon /> New site
              </Button>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Site</TableHead>
                      <TableHead className="hidden sm:table-cell">
                        Backends
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {remoteSites.map((remoteSite) => {
                      return (
                        <TableRow
                          key={remoteSite.host_name}
                          className="hover:cursor-pointer"
                          onClick={() => {
                            router.navigate({
                              to: `/site`,
                              search: {
                                tab: 1,
                                hostname: getUrlFriendlyUrl(
                                  remoteSite.host_name
                                ),
                              },
                            });
                          }}
                        >
                          <TableCell
                            className={
                              remoteSite.backends.length > 1 ? "align-top" : ""
                            }
                          >
                            <div className="font-bold">
                              {remoteSite.host_name}
                            </div>
                          </TableCell>
                          <TableCell className="hidden sm:table-cell">
                            <div className="text-sm text-muted-foreground overflow-hidden text-ellipsis max-w-[30ch]">
                              {remoteSite.backends.map((backend) => {
                                return (
                                  <div
                                    key={`${backend.address}:${backend.port}`}
                                  >
                                    {backend.address}:{backend.port}
                                  </div>
                                );
                              })}
                            </div>
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </TabsContent>


          <TabsContent value="servers">
            <Card x-chunk="dashboard-06-chunk-1">
              <CardHeader className="flex items-center justify-between flex-row">
                <div>
                  <CardTitle>Servers</CardTitle>
                <CardDescription>Viewing all directory servers</CardDescription>
                </div>
                <Button
                onClick={() => {
                  router.navigate({
                    to: "/new-dirserver",
                  });
                }}
                className="opacity-[.9] flex gap-2 border border-transparent hover:border-white/20 m-0"
                variant="ghost"
              >
                <PlusSquareIcon /> New server
              </Button>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Hostname</TableHead>
                      <TableHead className="hidden sm:table-cell">
                        Directory
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {dirServers.map((dirServer) => {
                      return (
                        <TableRow
                          key={dirServer.host_name}
                          className="hover:cursor-pointer"
                          onClick={() => {
                            router.navigate({
                              to: `/site`,
                              search: {
                                tab: 1,
                                hostname: getUrlFriendlyUrl(
                                  dirServer.host_name
                                ),
                              },
                            });
                          }}
                        >
                          <TableCell>
                            <div className="font-bold">
                              {dirServer.host_name}
                            </div>
                          </TableCell>
                          <TableCell className="hidden sm:table-cell">
                            <div className="text-sm text-muted-foreground overflow-hidden text-ellipsis max-w-[30ch]">
                              {dirServer.dir}
                            </div>
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </div>
    </main>
  );
};

export default HomePage;
