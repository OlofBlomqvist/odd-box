import { createRootRoute, Outlet, redirect, useMatches } from "@tanstack/react-router";
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
import { DirectoryServersMenuActions } from "@/components/drawer/directory_servers_menu_actions";
import DirServersList from "@/components/drawer/dir-servers-list";
import LiveEventStreamProvider from "@/providers/live_logs";
import ThemeContextProvider from "@/providers/theme";
import { getCookie } from "../utils/cookies";
import { useEffect, useRef } from "react";

// Configure QueryClient with better defaults to avoid excessive re-renders
const queryClient = new QueryClient();

export const Route = createRootRoute({
  component: Root,
  beforeLoad: ({ context:_, location }) => {
    // Skip auth check for login page
    if (location.pathname === "/login") {
      console.log("Skipping auth check for login page");
      return;
    }

    const password = getCookie("password");
    
    if (!password) {
      // No password cookie, redirect to login
      console.log("No password cookie found, redirecting to login");
      throw redirect({
        to: "/login",
        search: { redirect: location.pathname }
      });
    }
  },
});

function Root() {
  const matches = useMatches();
  const isLoginPage = matches.some(match => match.routeId.includes('/login'));
  
  // Track renders without causing a loop
  const renderCount = useRef(0);
  
  useEffect(() => {
    renderCount.current += 1;
    console.log('Root component rendered:', renderCount.current);
    
    // Log excessive renders
    if (renderCount.current > 10 && renderCount.current % 10 === 0) {
      console.warn(`Excessive renders (${renderCount.current}) detected in Root component`);
    }
  }, []); // Empty dependency array with useRef instead of useState
  
  
  // Only render the minimal UI needed for login page
  if (isLoginPage) {
    return (
      <ThemeContextProvider>
        <QueryClientProvider client={queryClient}>
          <div className="min-h-screen">
            <Toaster
              toastOptions={{
                style: {
                  background: "#333",
                  color: "#fff",
                },
              }}
            />
            <Outlet />
          </div>
        </QueryClientProvider>
      </ThemeContextProvider>
    );
  }
  
  // Full app layout for authenticated routes
  return (
    <>
      <ThemeContextProvider>
        <QueryClientProvider client={queryClient}>
          <LiveEventStreamProvider>
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
                <div className="my-4" />
                <ListHeader
                  menuActions={<DirectoryServersMenuActions />}
                  label="STATIC SITES"
                />
                <DirServersList />
              </SideDrawer>
              <Header />
              <div className="inner-content">
                <Outlet />
              </div>
              <Footer />
            </DrawerProvider>
            <ReactQueryDevtools initialIsOpen={false} />
          </LiveEventStreamProvider>
        </QueryClientProvider>
      </ThemeContextProvider>
    </>
  );
}