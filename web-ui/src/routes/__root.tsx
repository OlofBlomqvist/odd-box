import { createRootRoute, Outlet } from "@tanstack/react-router";
import "../global_tw.css";
import SideDrawer from "../components/drawer/drawer";
import MenuItem from "../components/drawer/menu_item";
import DrawerProvider from "../components/drawer/context";
import Header from "../components/header/header";
import Footer from "../components/footer/footer";
import { Toaster } from "react-hot-toast";
import HostedProcessesList from "../components/drawer/hosted-processes-list";
import ListHeader from "../components/drawer/sites-list-header";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { LayoutDashboardIcon, Logs, Settings } from "lucide-react";
import RemoteSitesList from "@/components/drawer/remote-sites-list";
import { HostedProcessesMenuActions } from "@/components/drawer/hosted_processes_menu_actions";
import { RemoteSitesMenuActions } from "@/components/drawer/remote_sites_menu_actions";
const queryClient = new QueryClient();

export const Route = createRootRoute({
  component: () => (
    <>
      <QueryClientProvider client={queryClient}>
        <DrawerProvider>
          <Toaster
            toastOptions={{
              style: {
                background: "#333",
                color: "#fff",
              },
            }}
          />
          <SideDrawer>
            <MenuItem
              title="Dashboard"
              to="/"
              icon={<LayoutDashboardIcon className="h-5 w-5" />}
            />
            <MenuItem
              title="Logs"
              to="/logs"
              icon={<Logs className="h-5 w-5" />}
            />
            <MenuItem
              title="Settings"
              to="/settings"
              icon={<Settings className="h-5 w-5" />}
            />

            <hr className="mx-2 my-4 bg-[#ffffff22]" />
            <ListHeader
              menuActions={<HostedProcessesMenuActions />}
              label="HOSTED PROCESSES"
            />
            <HostedProcessesList />
            <div className="my-4" />
            <ListHeader
              menuActions={<RemoteSitesMenuActions />}
              label="REMOTE SITES"
            />
            <RemoteSitesList />
          </SideDrawer>
          <Header />
          <div className="inner-content">
            <Outlet />
          </div>
          <Footer />
        </DrawerProvider>
        <ReactQueryDevtools initialIsOpen={false} />
      </QueryClientProvider>
    </>
  ),
});
