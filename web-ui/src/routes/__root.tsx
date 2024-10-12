import { createRootRoute, Outlet } from "@tanstack/react-router";
import "../global_tw.css";
import SideDrawer from "../components/drawer/drawer";
import MenuItem from "../components/drawer/menu_item";
import DrawerProvider from "../components/drawer/context";
import Header from "../components/header/header";
import Footer from "../components/footer/footer";
import { Toaster } from "react-hot-toast";
import SitesList from "../components/drawer/sites-list";
import SitesListHeader from "../components/drawer/sites-list-header";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { LayoutDashboardIcon, Logs, Settings } from "lucide-react";
const queryClient = new QueryClient();

export const Route = createRootRoute({
  component: () => (
    <>
      <QueryClientProvider client={queryClient}>
        <DrawerProvider>
          <Toaster />
          <SideDrawer>
            <MenuItem
              title="Dashboard"
              fontWeight="lighter"
              href="/"
              icon={<LayoutDashboardIcon className="h-5 w-5" />}
            />
            <MenuItem
              title="Logs"
              fontWeight="lighter"
              href="/logs"
              icon={<Logs className="h-5 w-5" />}
            />
            <MenuItem
              title="Settings"
              fontWeight="lighter"
              href="/settings"
              icon={<Settings className="h-5 w-5" />}
            />

            <hr className="mx-2 my-4 bg-[#ffffff22]" />
            <SitesListHeader />
            <SitesList />
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
