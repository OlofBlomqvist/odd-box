import "./home-styles.css";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { ActivityIcon, Ellipsis, GlobeIcon, PlusIcon } from "lucide-react";
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
const HomePage = () => {
  const { data: hostedProcesses } = useHostedSites();
  const { data: remoteSites } = useRemoteSites();
  const { data: siteStatus } = useSiteStatus();
  const router = useRouter();
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
              <Card>
                <CardHeader>
                  <CardTitle>Processes</CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="flex items-center justify-between">
                    <div className="text-4xl font-bold">
                      {hostedProcesses.length}
                    </div>
                    <div className="rounded-full bg-success px-3 py-1 text-success-foreground">
                      <ActivityIcon className="h-4 w-4" />
                    </div>
                  </div>
                  <div className="text-sm text-muted-foreground">
                    Total processes
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardHeader>
                  <CardTitle>Sites</CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="flex items-center justify-between">
                    <div className="text-4xl font-bold">
                      {remoteSites.length}
                    </div>
                    <div className="rounded-full bg-success px-3 py-1 text-success-foreground">
                      <GlobeIcon className="h-4 w-4" />
                    </div>
                  </div>
                  <div className="text-sm text-muted-foreground">
                    Total sites
                  </div>
                </CardContent>
              </Card>
            </div>
          </CardContent>
        </Card>

        <Tabs defaultValue="processes" className="pb-8">
          <div className="flex items-center">
            <TabsList>
              <TabsTrigger value="processes">Processes</TabsTrigger>
              <TabsTrigger value="sites">Sites</TabsTrigger>
            </TabsList>
            <div className="ml-auto flex items-center gap-2">
              <Button
                size="sm"
                variant="outline"
                className="h-8 gap-1"
                onClick={() => {
                  router.navigate({
                    to: "/new-site",
                    search: { type: "remote" },
                  });
                }}
              >
                <PlusIcon className="h-3.5 w-3.5" />
                <span className="sr-only sm:not-sr-only sm:whitespace-nowrap">
                  Add Site
                </span>
              </Button>
              <Button
                size="sm"
                variant="outline"
                className="h-8 gap-1"
                onClick={() => {
                  router.navigate({ to: "/new-process" });
                }}
              >
                <PlusIcon className="h-3.5 w-3.5" />
                <span className="sr-only sm:not-sr-only sm:whitespace-nowrap">
                  Add Process
                </span>
              </Button>
            </div>
          </div>

          <TabsContent value="processes">
            <Card x-chunk="dashboard-06-chunk-1">
              <CardHeader>
                <CardTitle>Processes</CardTitle>
                <CardDescription>
                  Viewing all hosted processes and their status.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Site</TableHead>
                      <TableHead className="text-right pr-6">Status</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {hostedProcesses.map((hostedProcess) => {
                      const state = siteStatus.find(
                        (x) => x.hostname === hostedProcess.host_name
                      )?.state;
                      return (
                        <TableRow key={hostedProcess.host_name}>
                          <TableCell>
                            <div className="font-bold text-base">
                              {hostedProcess.host_name}
                            </div>
                            <div className="text-sm text-muted-foreground">
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
                          <TableCell className="text-right">
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
                                      to: `/site/${hostedProcess.host_name.replaceAll("http://", "").replaceAll("https://", "")}`,
                                    });
                                  }}
                                >
                                  View
                                </DropdownMenuItem>
                                <DropdownMenuItem
                                  onClick={() => {
                                    router.navigate({
                                      to: `/site/${hostedProcess.host_name.replaceAll("http://", "").replaceAll("https://", "")}`,
                                      search: { tab: 1 },
                                    });
                                  }}
                                >
                                  Edit
                                </DropdownMenuItem>
                                <DropdownMenuItem
                                  onClick={() => {
                                    router.navigate({
                                      to: `/site/${hostedProcess.host_name.replaceAll("http://", "").replaceAll("https://", "")}`,
                                      search: { tab: 2 },
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
              <CardHeader>
                <CardTitle>Sites</CardTitle>
                <CardDescription>Viewing all remote sites</CardDescription>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Site</TableHead>
                      {/* <TableHead>Requests</TableHead>
                <TableHead>Errors</TableHead> */}
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {remoteSites.map((remoteSite) => {
                      return (
                        <TableRow key={remoteSite.host_name}>
                          <TableCell>
                            <div className="font-bold text-base">
                              {remoteSite.host_name}
                            </div>
                            {/* <div className="text-sm text-muted-foreground">
                    {remoteSite.host_name}
                  </div> */}
                          </TableCell>
                          <TableCell className="text-right">
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
                                      to: `/site/${remoteSite.host_name.replaceAll("http://", "").replaceAll("https://", "")}`,
                                    });
                                  }}
                                >
                                  Edit
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
        </Tabs>
      </div>
    </main>
  );
};

export default HomePage;
