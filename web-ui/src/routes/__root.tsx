import { createRootRoute, Outlet } from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/router-devtools";
import "../global_tw.css";
import SideDrawer from "../components/drawer/drawer";
import MenuItem from "../components/drawer/menu_item";
import DrawerProvider from "../components/drawer/context";
import Header from "../components/header/header";
import Footer from "../components/footer/footer";
import { CogIcon } from "../components/icons/cog";
import { Toaster } from "react-hot-toast";
import SitesList from "../components/drawer/sites-list";
import SitesListHeader from "../components/drawer/sites-list-header";
import { HouseIcon } from "../components/icons/house";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import LogsIcon from "../components/icons/log";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
const queryClient = new QueryClient();

export const Route = createRootRoute({
  component: () => (
    <>
      <QueryClientProvider client={queryClient}>
        <DrawerProvider>
          <Toaster />
          <SideDrawer>
            <MenuItem
              title="Home"
              fontWeight="lighter"
              href="/"
              icon={<HouseIcon />}
            />
            <MenuItem
              title="Settings"
              fontWeight="lighter"
              href="/settings"
              icon={<CogIcon />}
            />
            <MenuItem
              title="Logs"
              fontWeight="lighter"
              href="/logs"
              icon={<LogsIcon />}
            />
            <hr style={{ margin: "15px 5px", opacity: 0.2 }} />
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
        <TanStackRouterDevtools position="bottom-right" />
      </QueryClientProvider>
    </>
  ),
});
